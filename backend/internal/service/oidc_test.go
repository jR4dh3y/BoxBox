package service

import (
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"net/url"
	"sync/atomic"
	"testing"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/oidctest"
)

const testOIDCRedirect = "http://boxbox.test/api/v1/auth/oidc/callback"

func TestOIDCDiscoveryCooldownStartsAfterFailure(t *testing.T) {
	var requests atomic.Int32
	failedAt := make(chan time.Time, 1)
	issuer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		requests.Add(1)
		failedAt <- time.Now()
		http.Error(w, "provider unavailable", http.StatusServiceUnavailable)
	}))
	defer issuer.Close()
	svc := NewOIDCService(OIDCServiceConfig{IssuerURL: issuer.URL})
	if _, _, err := svc.Begin(context.Background()); !errors.Is(err, ErrOIDCUnavailable) {
		t.Fatalf("discovery error = %v", err)
	}
	if svc.lastAttempt.Before(<-failedAt) {
		t.Fatal("discovery cooldown started before the provider failed")
	}
	if _, _, err := svc.Begin(context.Background()); !errors.Is(err, ErrOIDCUnavailable) {
		t.Fatalf("cooldown error = %v", err)
	}
	if requests.Load() != 1 {
		t.Fatal("discovery failure was not shared during cooldown")
	}
}

func newTestOIDC(t *testing.T, claim string) (*OIDCService, *oidctest.Issuer) {
	t.Helper()
	issuer := oidctest.New(t)
	return NewOIDCService(OIDCServiceConfig{
		IssuerURL:     issuer.URL(),
		ClientID:      oidctest.ClientID,
		ClientSecret:  oidctest.ClientSecret,
		RedirectURL:   testOIDCRedirect,
		UsernameClaim: claim,
		Scopes:        []string{"openid", "profile", "email"},
		JWTSecret:     "test-secret", // the same secret as newTestAuthService
		HTTPClient:    issuer.Client(),
	}), issuer
}

// signIn runs Begin, the provider's approval and Complete.
func signIn(t *testing.T, svc *OIDCService, issuer *oidctest.Issuer, edit func(*OIDCCallback)) (string, error) {
	t.Helper()
	authURL, flow, err := svc.Begin(context.Background())
	if err != nil {
		t.Fatalf("Begin() error = %v", err)
	}
	code, state := issuer.Authorize(t, authURL)
	callback := OIDCCallback{State: state, Code: code}
	if edit != nil {
		edit(&callback)
	}
	return svc.Complete(context.Background(), flow, callback)
}

func TestOIDCSignInReturnsUsernameClaim(t *testing.T) {
	svc, issuer := newTestOIDC(t, "preferred_username")
	issuer.Claims = map[string]any{"preferred_username": "Alice"}

	authURL, _, err := svc.Begin(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	query := mustQuery(t, authURL)
	if query.Get("redirect_uri") != testOIDCRedirect || query.Get("scope") != "openid profile email" || query.Get("state") == "" {
		t.Fatalf("authorization URL = %s", authURL)
	}

	username, err := signIn(t, svc, issuer, nil)
	if err != nil || username != "Alice" {
		t.Fatalf("Complete() = %q, %v; want Alice", username, err)
	}
}

func TestOIDCRejectsFlowProblemsBeforeTokenExchange(t *testing.T) {
	svc, issuer := newTestOIDC(t, "preferred_username")
	issuer.Claims = map[string]any{"preferred_username": "alice"}
	_, otherFlow, err := svc.Begin(context.Background())
	if err != nil {
		t.Fatal(err)
	}

	tests := map[string]func(*OIDCCallback){
		"state mismatch": func(c *OIDCCallback) { c.State = "other" },
		"missing state":  func(c *OIDCCallback) { c.State = "" },
		"missing code":   func(c *OIDCCallback) { c.Code = "" },
	}
	for name, edit := range tests {
		if _, err := signIn(t, svc, issuer, edit); !errors.Is(err, ErrOIDCFlow) {
			t.Errorf("%s: err = %v, want ErrOIDCFlow", name, err)
		}
	}
	for name, flow := range map[string]string{
		"tampered":    otherFlow[:len(otherFlow)-2] + "xx",
		"garbage":     "not-a-token",
		"other key":   mustSessionToken(t),
		"empty value": "",
	} {
		if _, err := svc.Complete(context.Background(), flow, OIDCCallback{State: "s", Code: "c"}); !errors.Is(err, ErrOIDCFlow) {
			t.Errorf("%s flow: err = %v, want ErrOIDCFlow", name, err)
		}
	}
	if issuer.TokenRequests != 0 {
		t.Fatalf("token endpoint called %d times for rejected flows", issuer.TokenRequests)
	}
}

func TestOIDCFlowTokenExpires(t *testing.T) {
	svc, _ := newTestOIDC(t, "preferred_username")
	expired, err := svc.sealFlow(oidcFlow{State: "s", Nonce: "n", Verifier: "v"})
	if err != nil {
		t.Fatal(err)
	}
	flow, err := svc.openFlow(expired)
	if err != nil || flow.State != "s" {
		t.Fatalf("fresh flow = %+v, %v", flow, err)
	}
	flow.ExpiresAt.Time = time.Now().Add(-time.Minute)
	stale, err := signFlowAsIs(svc, flow)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := svc.openFlow(stale); !errors.Is(err, ErrOIDCFlow) {
		t.Fatalf("expired flow err = %v", err)
	}
}

func TestOIDCProviderAnswers(t *testing.T) {
	tests := []struct {
		name  string
		setup func(*oidctest.Issuer)
		edit  func(*OIDCCallback)
		want  error
	}{
		{name: "access denied", edit: func(c *OIDCCallback) { c.Error, c.Code = "access_denied", "" }, want: ErrOIDCDenied},
		{name: "other provider error", edit: func(c *OIDCCallback) { c.Error = "server_error" }, want: ErrOIDCProvider},
		{name: "replayed or bad code", edit: func(c *OIDCCallback) { c.Code = "unknown" }, want: ErrOIDCProvider},
		{name: "missing id_token", setup: func(i *oidctest.Issuer) { i.OmitIDToken = true }, want: ErrOIDCProvider},
		{name: "unknown signing key", setup: func(i *oidctest.Issuer) { i.SignWithOtherKey = true }, want: ErrOIDCProvider},
		{name: "wrong audience", setup: func(i *oidctest.Issuer) { i.Claims["aud"] = "someone-else" }, want: ErrOIDCProvider},
		{name: "wrong issuer", setup: func(i *oidctest.Issuer) { i.Claims["iss"] = "https://evil.test" }, want: ErrOIDCProvider},
		{name: "expired", setup: func(i *oidctest.Issuer) { i.Claims["exp"] = time.Now().Add(-time.Hour).Unix() }, want: ErrOIDCProvider},
		{name: "azp of another client", setup: func(i *oidctest.Issuer) { i.Claims["azp"] = "other-client" }, want: ErrOIDCProvider},
		{name: "two audiences without azp", setup: func(i *oidctest.Issuer) { i.Claims["aud"] = []string{oidctest.ClientID, "other"} }, want: ErrOIDCProvider},
		{name: "empty subject", setup: func(i *oidctest.Issuer) { i.Claims["sub"] = "" }, want: ErrOIDCProvider},
		{name: "nonce mismatch", setup: func(i *oidctest.Issuer) { i.Claims["nonce"] = "replayed" }, want: ErrOIDCFlow},
		{name: "two audiences with our azp", setup: func(i *oidctest.Issuer) {
			i.Claims["aud"] = []string{oidctest.ClientID, "other"}
			i.Claims["azp"] = oidctest.ClientID
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			svc, issuer := newTestOIDC(t, "preferred_username")
			issuer.Claims = map[string]any{"preferred_username": "alice"}
			if test.setup != nil {
				test.setup(issuer)
			}
			username, err := signIn(t, svc, issuer, test.edit)
			if test.want == nil {
				if err != nil || username != "alice" {
					t.Fatalf("Complete() = %q, %v", username, err)
				}
				return
			}
			if !errors.Is(err, test.want) {
				t.Fatalf("err = %v, want %v", err, test.want)
			}
		})
	}
}

func TestOIDCUsernameClaimHandling(t *testing.T) {
	tests := []struct {
		name     string
		claim    string
		claims   map[string]any
		userInfo map[string]any
		want     string
		wantErr  error
	}{
		{name: "from userinfo", claim: "preferred_username", claims: map[string]any{}, userInfo: map[string]any{"preferred_username": "bob"}, want: "bob"},
		{name: "userinfo subject mismatch", claim: "preferred_username", claims: map[string]any{}, userInfo: map[string]any{"sub": "user-2", "preferred_username": "bob"}, wantErr: ErrOIDCProvider},
		{name: "absent everywhere", claim: "preferred_username", claims: map[string]any{}, wantErr: ErrOIDCIdentity},
		{name: "not a string", claim: "preferred_username", claims: map[string]any{"preferred_username": 42}, wantErr: ErrOIDCIdentity},
		{name: "control character", claim: "preferred_username", claims: map[string]any{"preferred_username": "al\nice"}, wantErr: ErrOIDCIdentity},
		{name: "surrounding space", claim: "preferred_username", claims: map[string]any{"preferred_username": " alice"}, wantErr: ErrOIDCIdentity},
		{name: "verified email", claim: "email", claims: map[string]any{"email": "a@example.com", "email_verified": true}, want: "a@example.com"},
		{name: "verified email as string", claim: "email", claims: map[string]any{"email": "a@example.com", "email_verified": "true"}, want: "a@example.com"},
		{name: "unverified email", claim: "email", claims: map[string]any{"email": "a@example.com", "email_verified": false}, wantErr: ErrOIDCIdentity},
		{name: "email without verification", claim: "email", claims: map[string]any{"email": "a@example.com"}, wantErr: ErrOIDCIdentity},
		{name: "userinfo email without its own verification", claim: "email", claims: map[string]any{"email_verified": true}, userInfo: map[string]any{"email": "a@example.com"}, wantErr: ErrOIDCIdentity},
		{name: "verified userinfo email", claim: "email", claims: map[string]any{}, userInfo: map[string]any{"email": "a@example.com", "email_verified": true}, want: "a@example.com"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			svc, issuer := newTestOIDC(t, test.claim)
			issuer.Claims = test.claims
			issuer.UserInfo = test.userInfo
			username, err := signIn(t, svc, issuer, nil)
			if test.wantErr != nil {
				if !errors.Is(err, test.wantErr) {
					t.Fatalf("err = %v, want %v", err, test.wantErr)
				}
				return
			}
			if err != nil || username != test.want {
				t.Fatalf("Complete() = %q, %v; want %q", username, err, test.want)
			}
		})
	}
}

// A provider that is down at startup is retried on a later sign-in, without a restart.
func TestOIDCSignInAfterProviderRecovers(t *testing.T) {
	svc, issuer := newTestOIDC(t, "preferred_username")
	issuer.Claims = map[string]any{"preferred_username": "alice"}
	issuer.FailDiscovery = true
	svc.Warm()
	if svc.provider != nil {
		t.Fatal("discovery succeeded while the provider was down")
	}

	issuer.FailDiscovery = false
	if _, _, err := svc.Begin(context.Background()); !errors.Is(err, ErrOIDCUnavailable) {
		t.Fatalf("sign-in inside the cooldown err = %v, want ErrOIDCUnavailable", err)
	}
	// Rewind the clock instead of sleeping through the cooldown.
	svc.lastAttempt = time.Now().Add(-oidcRetryInterval)
	username, err := signIn(t, svc, issuer, nil)
	if err != nil || username != "alice" {
		t.Fatalf("sign-in after recovery = %q, %v; want alice", username, err)
	}
}

func TestOIDCFlowTokenIsNotASessionToken(t *testing.T) {
	svc, _ := newTestOIDC(t, "preferred_username")
	flow, err := svc.sealFlow(oidcFlow{State: "s", Nonce: "n", Verifier: "v"})
	if err != nil {
		t.Fatal(err)
	}
	if _, err := newTestAuthService(t).ValidateToken(flow); err == nil {
		t.Fatal("a flow token validated as a session token")
	}
}

// mustSessionToken returns a real session token signed with the same JWT secret.
func mustSessionToken(t *testing.T) string {
	t.Helper()
	pair, err := newTestAuthService(t).Login(context.Background(), "admin", "correct-password")
	if err != nil {
		t.Fatal(err)
	}
	return pair.AccessToken
}

func signFlowAsIs(svc *OIDCService, flow oidcFlow) (string, error) {
	return jwt.NewWithClaims(jwt.SigningMethodHS256, &flow).SignedString(svc.flowKey)
}

func mustQuery(t *testing.T, raw string) url.Values {
	t.Helper()
	parsed, err := url.Parse(raw)
	if err != nil {
		t.Fatal(err)
	}
	return parsed.Query()
}
