package service

import (
	"context"
	"crypto/rand"
	"crypto/subtle"
	"encoding/base64"
	"encoding/json"
	"errors"
	"net/http"
	"sync"
	"time"

	"github.com/coreos/go-oidc/v3/oidc"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/rs/zerolog/log"
	"golang.org/x/oauth2"
)

var (
	ErrOIDCUnavailable = errors.New("identity provider unavailable")
	ErrOIDCFlow        = errors.New("sign-in flow invalid or expired")
	ErrOIDCDenied      = errors.New("sign-in denied by identity provider")
	ErrOIDCProvider    = errors.New("identity provider rejected sign-in")
	ErrOIDCIdentity    = errors.New("identity has no usable username claim")
)

const (
	oidcHTTPTimeout   = 10 * time.Second
	oidcRetryInterval = 10 * time.Second
)

// OIDCServiceConfig configures the OpenID Connect client.
type OIDCServiceConfig struct {
	IssuerURL     string
	ClientID      string
	ClientSecret  string
	RedirectURL   string
	UsernameClaim string
	Scopes        []string
	JWTSecret     string
	HTTPClient    *http.Client // nil uses a client with a 10 s timeout
}

// OIDCCallback is what the identity provider sends back to the callback URL.
type OIDCCallback struct {
	State string
	Code  string
	Error string
}

// OIDCService runs the authorization code flow with PKCE, state and nonce.
type OIDCService struct {
	cfg        OIDCServiceConfig
	flowKey    []byte
	httpClient *http.Client

	mu          sync.Mutex
	provider    *oidc.Provider
	lastAttempt time.Time
}

// NewOIDCService builds the client without network I/O; discovery happens on first use.
func NewOIDCService(cfg OIDCServiceConfig) *OIDCService {
	client := cfg.HTTPClient
	if client == nil {
		client = &http.Client{Timeout: oidcHTTPTimeout}
	}
	return &OIDCService{cfg: cfg, flowKey: oidcFlowKey(cfg.JWTSecret), httpClient: client}
}

// Warm tries discovery once so configuration problems show up in the startup log.
func (s *OIDCService) Warm() {
	if _, err := s.discover(); err != nil {
		log.Warn().Err(err).Str("issuer", s.cfg.IssuerURL).Msg("OIDC provider discovery failed; retrying on next sign-in")
		return
	}
	log.Info().Str("issuer", s.cfg.IssuerURL).Msg("OIDC provider discovered")
}

// discover is lazy so an unreachable provider never stops BoxBox or password login.
func (s *OIDCService) discover() (*oidc.Provider, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.provider != nil {
		return s.provider, nil
	}
	if time.Since(s.lastAttempt) < oidcRetryInterval {
		return nil, ErrOIDCUnavailable
	}
	s.lastAttempt = time.Now()
	// A background context: the provider keeps this client for later key fetches.
	provider, err := oidc.NewProvider(oidc.ClientContext(context.Background(), s.httpClient), s.cfg.IssuerURL)
	if err != nil {
		return nil, errors.Join(ErrOIDCUnavailable, err)
	}
	s.provider = provider
	return provider, nil
}

func (s *OIDCService) oauthConfig(provider *oidc.Provider) oauth2.Config {
	return oauth2.Config{
		ClientID:     s.cfg.ClientID,
		ClientSecret: s.cfg.ClientSecret,
		Endpoint:     provider.Endpoint(),
		RedirectURL:  s.cfg.RedirectURL,
		Scopes:       s.cfg.Scopes,
	}
}

// Begin returns the provider URL to send the browser to and the sealed flow for its cookie.
func (s *OIDCService) Begin(ctx context.Context) (authURL, flowToken string, err error) {
	provider, err := s.discover()
	if err != nil {
		return "", "", err
	}
	flow := oidcFlow{State: randomToken(), Nonce: randomToken(), Verifier: oauth2.GenerateVerifier()}
	flowToken, err = s.sealFlow(flow)
	if err != nil {
		return "", "", err
	}
	config := s.oauthConfig(provider)
	authURL = config.AuthCodeURL(flow.State, oidc.Nonce(flow.Nonce), oauth2.S256ChallengeOption(flow.Verifier))
	return authURL, flowToken, nil
}

// Complete checks the callback against the flow and returns the verified username.
func (s *OIDCService) Complete(ctx context.Context, flowToken string, callback OIDCCallback) (string, error) {
	flow, err := s.openFlow(flowToken)
	if err != nil {
		return "", err
	}
	if callback.State == "" || subtle.ConstantTimeCompare([]byte(callback.State), []byte(flow.State)) != 1 {
		return "", ErrOIDCFlow
	}
	if callback.Error != "" {
		log.Warn().Str("error", truncate(callback.Error, 64)).Msg("OIDC provider returned an error")
		if callback.Error == "access_denied" {
			return "", ErrOIDCDenied
		}
		return "", ErrOIDCProvider
	}
	if callback.Code == "" {
		return "", ErrOIDCFlow
	}
	provider, err := s.discover()
	if err != nil {
		return "", err
	}

	// Token and userinfo requests use the timeout client, not http.DefaultClient.
	ctx = oidc.ClientContext(ctx, s.httpClient)
	config := s.oauthConfig(provider)
	token, err := config.Exchange(ctx, callback.Code, oauth2.VerifierOption(flow.Verifier))
	if err != nil {
		return "", exchangeError(err)
	}
	rawIDToken, ok := token.Extra("id_token").(string)
	if !ok || rawIDToken == "" {
		return "", ErrOIDCProvider
	}
	idToken, err := provider.Verifier(&oidc.Config{ClientID: s.cfg.ClientID}).Verify(ctx, rawIDToken)
	if err != nil {
		log.Warn().Err(err).Msg("OIDC ID token rejected")
		return "", ErrOIDCProvider
	}
	if subtle.ConstantTimeCompare([]byte(idToken.Nonce), []byte(flow.Nonce)) != 1 {
		return "", ErrOIDCFlow
	}
	var claims map[string]json.RawMessage
	if err := idToken.Claims(&claims); err != nil || idToken.Subject == "" {
		return "", ErrOIDCProvider
	}
	if !s.authorizedParty(claims, idToken.Audience) {
		return "", ErrOIDCProvider
	}

	if _, present := claims[s.cfg.UsernameClaim]; !present && provider.UserInfoEndpoint() != "" {
		userInfo, err := provider.UserInfo(ctx, oauth2.StaticTokenSource(token))
		if err != nil {
			log.Warn().Err(err).Msg("OIDC userinfo request failed")
			return "", ErrOIDCUnavailable
		}
		if userInfo.Subject != idToken.Subject {
			return "", ErrOIDCProvider
		}
		if err := userInfo.Claims(&claims); err != nil {
			return "", ErrOIDCProvider
		}
	}
	return usernameFromClaims(claims, s.cfg.UsernameClaim)
}

// authorizedParty applies OIDC Core 3.1.3.7: azp, when present, must be this client.
func (s *OIDCService) authorizedParty(claims map[string]json.RawMessage, audience []string) bool {
	raw, present := claims["azp"]
	if !present {
		return len(audience) <= 1
	}
	var azp string
	return json.Unmarshal(raw, &azp) == nil && azp == s.cfg.ClientID
}

func usernameFromClaims(claims map[string]json.RawMessage, claim string) (string, error) {
	raw, present := claims[claim]
	if !present {
		return "", ErrOIDCIdentity
	}
	var username string
	if err := json.Unmarshal(raw, &username); err != nil || !model.ValidUsername(username) {
		return "", ErrOIDCIdentity
	}
	if claim == "email" && !emailVerified(claims["email_verified"]) {
		return "", ErrOIDCIdentity
	}
	return username, nil
}

// emailVerified accepts true, and the string "true" that some providers send.
func emailVerified(raw json.RawMessage) bool {
	var verified bool
	if json.Unmarshal(raw, &verified) == nil {
		return verified
	}
	var text string
	return json.Unmarshal(raw, &text) == nil && text == "true"
}

func exchangeError(err error) error {
	var retrieve *oauth2.RetrieveError
	if errors.As(err, &retrieve) && retrieve.Response != nil && retrieve.Response.StatusCode < http.StatusInternalServerError {
		log.Warn().Str("error", truncate(retrieve.ErrorCode, 64)).Msg("OIDC token exchange rejected")
		return ErrOIDCProvider
	}
	log.Warn().Err(err).Msg("OIDC token exchange failed")
	return ErrOIDCUnavailable
}

func randomToken() string {
	b := make([]byte, 32)
	_, _ = rand.Read(b)
	return base64.RawURLEncoding.EncodeToString(b)
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n]
}
