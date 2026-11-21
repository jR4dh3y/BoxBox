// Package oidctest is a fake OpenID Connect provider for tests.
package oidctest

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"math/big"
	"net/http"
	"net/http/httptest"
	"net/url"
	"sync"
	"testing"
	"time"

	"github.com/golang-jwt/jwt/v5"
)

const (
	ClientID     = "boxbox-test"
	ClientSecret = "test-secret"
	Subject      = "user-1"
	accessToken  = "test-access-token"
	keyID        = "test-key"
)

// Issuer serves discovery, JWKS, authorize, token and userinfo. Set the exported
// fields before the flow under test to change what it returns.
type Issuer struct {
	server   *httptest.Server
	key      *rsa.PrivateKey
	otherKey *rsa.PrivateKey

	mu sync.Mutex
	// Claims are added to (and override) the ID token claims.
	Claims map[string]any
	// UserInfo is the userinfo response; sub defaults to Subject.
	UserInfo map[string]any
	// OmitIDToken drops id_token from the token response.
	OmitIDToken bool
	// SignWithOtherKey signs the ID token with a key missing from the JWKS.
	SignWithOtherKey bool
	// FailDiscovery answers discovery with 500.
	FailDiscovery bool
	codes         map[string]authRequest
	TokenRequests int
}

type authRequest struct {
	nonce, challenge, redirectURI string
}

// New starts an issuer that stops with the test.
func New(t *testing.T) *Issuer {
	t.Helper()
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatal(err)
	}
	otherKey, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatal(err)
	}
	issuer := &Issuer{key: key, otherKey: otherKey, codes: map[string]authRequest{}}
	mux := http.NewServeMux()
	mux.HandleFunc("GET /.well-known/openid-configuration", issuer.discovery)
	mux.HandleFunc("GET /jwks", issuer.jwks)
	mux.HandleFunc("POST /token", issuer.token)
	mux.HandleFunc("GET /userinfo", issuer.userinfo)
	issuer.server = httptest.NewServer(mux)
	t.Cleanup(issuer.server.Close)
	return issuer
}

// URL is the issuer identifier.
func (i *Issuer) URL() string { return i.server.URL }

// Client is an HTTP client for the issuer.
func (i *Issuer) Client() *http.Client { return i.server.Client() }

// Authorize plays the user approving the request at authURL and returns the callback code and state.
func (i *Issuer) Authorize(t *testing.T, authURL string) (code, state string) {
	t.Helper()
	parsed, err := url.Parse(authURL)
	if err != nil {
		t.Fatal(err)
	}
	query := parsed.Query()
	if query.Get("client_id") != ClientID || query.Get("response_type") != "code" ||
		query.Get("code_challenge_method") != "S256" || query.Get("code_challenge") == "" || query.Get("nonce") == "" {
		t.Fatalf("authorization request is missing PKCE, nonce or client: %s", authURL)
	}
	code = base64.RawURLEncoding.EncodeToString([]byte(time.Now().String() + query.Get("state")))
	i.mu.Lock()
	i.codes[code] = authRequest{nonce: query.Get("nonce"), challenge: query.Get("code_challenge"), redirectURI: query.Get("redirect_uri")}
	i.mu.Unlock()
	return code, query.Get("state")
}

func (i *Issuer) discovery(w http.ResponseWriter, _ *http.Request) {
	i.mu.Lock()
	fail := i.FailDiscovery
	i.mu.Unlock()
	if fail {
		http.Error(w, "down", http.StatusInternalServerError)
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"issuer":                                i.URL(),
		"authorization_endpoint":                i.URL() + "/authorize",
		"token_endpoint":                        i.URL() + "/token",
		"userinfo_endpoint":                     i.URL() + "/userinfo",
		"jwks_uri":                              i.URL() + "/jwks",
		"response_types_supported":              []string{"code"},
		"subject_types_supported":               []string{"public"},
		"id_token_signing_alg_values_supported": []string{"RS256"},
	})
}

func (i *Issuer) jwks(w http.ResponseWriter, _ *http.Request) {
	public := i.key.PublicKey
	writeJSON(w, http.StatusOK, map[string]any{"keys": []map[string]string{{
		"kty": "RSA", "alg": "RS256", "use": "sig", "kid": keyID,
		"n": base64.RawURLEncoding.EncodeToString(public.N.Bytes()),
		"e": base64.RawURLEncoding.EncodeToString(big.NewInt(int64(public.E)).Bytes()),
	}}})
}

func (i *Issuer) token(w http.ResponseWriter, r *http.Request) {
	i.mu.Lock()
	defer i.mu.Unlock()
	i.TokenRequests++
	if err := r.ParseForm(); err != nil {
		tokenError(w, "invalid_request")
		return
	}
	clientID, secret, ok := r.BasicAuth()
	if !ok {
		clientID, secret = r.PostForm.Get("client_id"), r.PostForm.Get("client_secret")
	}
	if clientID != ClientID || secret != ClientSecret {
		tokenError(w, "invalid_client")
		return
	}
	request, found := i.codes[r.PostForm.Get("code")]
	delete(i.codes, r.PostForm.Get("code")) // single use
	verifierHash := sha256.Sum256([]byte(r.PostForm.Get("code_verifier")))
	if !found || r.PostForm.Get("grant_type") != "authorization_code" ||
		r.PostForm.Get("redirect_uri") != request.redirectURI ||
		base64.RawURLEncoding.EncodeToString(verifierHash[:]) != request.challenge {
		tokenError(w, "invalid_grant")
		return
	}

	response := map[string]any{"access_token": accessToken, "token_type": "Bearer", "expires_in": 3600}
	if !i.OmitIDToken {
		now := time.Now()
		claims := jwt.MapClaims{
			"iss": i.URL(), "sub": Subject, "aud": ClientID, "nonce": request.nonce,
			"iat": now.Unix(), "exp": now.Add(time.Hour).Unix(),
		}
		for name, value := range i.Claims {
			if value == nil {
				delete(claims, name)
				continue
			}
			claims[name] = value
		}
		token := jwt.NewWithClaims(jwt.SigningMethodRS256, claims)
		token.Header["kid"] = keyID
		signingKey := i.key
		if i.SignWithOtherKey {
			signingKey = i.otherKey
		}
		signed, err := token.SignedString(signingKey)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		response["id_token"] = signed
	}
	writeJSON(w, http.StatusOK, response)
}

func (i *Issuer) userinfo(w http.ResponseWriter, r *http.Request) {
	if r.Header.Get("Authorization") != "Bearer "+accessToken {
		w.WriteHeader(http.StatusUnauthorized)
		return
	}
	i.mu.Lock()
	info := map[string]any{"sub": Subject}
	for name, value := range i.UserInfo {
		info[name] = value
	}
	i.mu.Unlock()
	writeJSON(w, http.StatusOK, info)
}

func tokenError(w http.ResponseWriter, code string) {
	writeJSON(w, http.StatusBadRequest, map[string]string{"error": code})
}

func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}
