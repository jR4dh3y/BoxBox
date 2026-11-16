package handler

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/oidctest"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type oidcFixture struct {
	issuer *oidctest.Issuer
	router *chi.Mux
	auth   service.AuthService
}

func newOIDCFixture(t *testing.T, redirectURL string, mutate func(*service.AuthServiceConfig)) oidcFixture {
	t.Helper()
	issuer := oidctest.New(t)
	cfg := authConfig(t, t.TempDir())
	cfg.SSOUsers = []string{"carol"}
	if mutate != nil {
		mutate(&cfg)
	}
	auth, err := service.NewAuthService(filesystem.NewOsFS(), cfg)
	if err != nil {
		t.Fatal(err)
	}
	flow := service.NewOIDCService(service.OIDCServiceConfig{
		IssuerURL:     issuer.URL(),
		ClientID:      oidctest.ClientID,
		ClientSecret:  oidctest.ClientSecret,
		RedirectURL:   redirectURL,
		UsernameClaim: "preferred_username",
		Scopes:        []string{"openid", "profile"},
		JWTSecret:     cfg.JWTSecret,
		HTTPClient:    issuer.Client(),
	})
	r := chi.NewRouter()
	r.Route("/api/v1/auth", func(r chi.Router) {
		r.Get("/config", AuthConfig(AuthConfigResponse{PasswordLogin: true, OIDC: &AuthConfigOIDC{Name: "Authelia"}}))
		NewAuthHandler(auth, redirectURL).RegisterRoutes(r)
		NewOIDCHandler(flow, auth, redirectURL).RegisterRoutes(r)
	})
	return oidcFixture{issuer: issuer, router: r, auth: auth}
}

func (f oidcFixture) do(t *testing.T, method, target string, cookies ...*http.Cookie) *httptest.ResponseRecorder {
	t.Helper()
	req := httptest.NewRequest(method, target, nil)
	for _, cookie := range cookies {
		req.AddCookie(cookie)
	}
	rec := httptest.NewRecorder()
	f.router.ServeHTTP(rec, req)
	return rec
}

func cookieNamed(rec *httptest.ResponseRecorder, name string) *http.Cookie {
	for _, cookie := range rec.Result().Cookies() {
		if cookie.Name == name {
			return cookie
		}
	}
	return nil
}

// startSignIn follows /oidc/login and the provider's approval; it returns the flow cookie and callback URL.
func (f oidcFixture) startSignIn(t *testing.T, flowCookieName string) (*http.Cookie, string) {
	t.Helper()
	rec := f.do(t, http.MethodGet, "http://boxbox.test/api/v1/auth/oidc/login")
	if rec.Code != http.StatusFound || rec.Header().Get("Cache-Control") != "no-store" {
		t.Fatalf("login = %d %v", rec.Code, rec.Header())
	}
	flow := cookieNamed(rec, flowCookieName)
	if flow == nil || !flow.HttpOnly || flow.SameSite != http.SameSiteLaxMode || flow.MaxAge != 600 {
		t.Fatalf("flow cookie = %#v", flow)
	}
	code, state := f.issuer.Authorize(t, rec.Header().Get("Location"))
	return flow, "/api/v1/auth/oidc/callback?" + url.Values{"code": {code}, "state": {state}}.Encode()
}

func ssoErrorParam(t *testing.T, rec *httptest.ResponseRecorder) string {
	t.Helper()
	if rec.Code != http.StatusSeeOther {
		t.Fatalf("status = %d, want 303", rec.Code)
	}
	location, err := url.Parse(rec.Header().Get("Location"))
	if err != nil || location.Path != "/login" || location.Host != "" {
		t.Fatalf("Location = %q", rec.Header().Get("Location"))
	}
	if cookieNamed(rec, refreshCookieName) != nil {
		t.Fatal("a failed sign-in set a refresh cookie")
	}
	return location.Query().Get("sso_error")
}

func TestOIDCSignInRoundTrip(t *testing.T) {
	f := newOIDCFixture(t, "http://boxbox.test/api/v1/auth/oidc/callback", nil)
	f.issuer.Claims = map[string]any{"preferred_username": "carol"}

	flow, callback := f.startSignIn(t, "boxbox_oidc_flow")
	if flow.Secure || flow.Path != "/api/v1/auth/oidc" {
		t.Fatalf("http flow cookie = %#v", flow)
	}
	rec := f.do(t, http.MethodGet, callback+"&next=https://evil.example", flow)
	if rec.Code != http.StatusSeeOther || rec.Header().Get("Location") != "/browse" {
		t.Fatalf("callback = %d %q", rec.Code, rec.Header().Get("Location"))
	}
	if cleared := cookieNamed(rec, "boxbox_oidc_flow"); cleared == nil || cleared.MaxAge >= 0 {
		t.Fatalf("flow cookie not cleared: %#v", cleared)
	}
	refresh := cookieNamed(rec, refreshCookieName)
	if refresh == nil || !refresh.HttpOnly || refresh.SameSite != http.SameSiteStrictMode || refresh.Path != "/api/v1/auth" {
		t.Fatalf("refresh cookie = %#v", refresh)
	}

	rec = f.do(t, http.MethodPost, "http://boxbox.test/api/v1/auth/refresh", refresh)
	var body LoginResponse
	if rec.Code != http.StatusOK || json.Unmarshal(rec.Body.Bytes(), &body) != nil {
		t.Fatalf("refresh = %d %s", rec.Code, rec.Body.String())
	}
	if claims, err := f.auth.ValidateAccessToken(body.AccessToken); err != nil || claims.Username != "carol" {
		t.Fatalf("access token = %+v, %v", claims, err)
	}

	if code := ssoErrorParam(t, f.do(t, http.MethodGet, callback, flow)); code != "provider_error" {
		t.Fatalf("replayed callback code = %q, want provider_error", code)
	}
}

func TestOIDCCallbackFailuresRedirectToLogin(t *testing.T) {
	tests := []struct {
		name string
		run  func(t *testing.T, f oidcFixture) *httptest.ResponseRecorder
		want string
	}{
		{name: "no flow cookie", want: "flow_expired", run: func(t *testing.T, f oidcFixture) *httptest.ResponseRecorder {
			_, callback := f.startSignIn(t, "boxbox_oidc_flow")
			return f.do(t, http.MethodGet, callback)
		}},
		{name: "state mismatch", want: "flow_expired", run: func(t *testing.T, f oidcFixture) *httptest.ResponseRecorder {
			flow, callback := f.startSignIn(t, "boxbox_oidc_flow")
			return f.do(t, http.MethodGet, strings.Replace(callback, "state=", "state=x", 1), flow)
		}},
		{name: "another browser's flow", want: "flow_expired", run: func(t *testing.T, f oidcFixture) *httptest.ResponseRecorder {
			other, _ := f.startSignIn(t, "boxbox_oidc_flow")
			_, callback := f.startSignIn(t, "boxbox_oidc_flow")
			return f.do(t, http.MethodGet, callback, other)
		}},
		{name: "denied at provider", want: "denied", run: func(t *testing.T, f oidcFixture) *httptest.ResponseRecorder {
			flow, callback := f.startSignIn(t, "boxbox_oidc_flow")
			state := strings.SplitN(callback, "state=", 2)[1]
			return f.do(t, http.MethodGet, "/api/v1/auth/oidc/callback?error=access_denied&error_description=%3Cscript%3E&state="+state, flow)
		}},
		{name: "account not configured", want: "not_allowed", run: func(t *testing.T, f oidcFixture) *httptest.ResponseRecorder {
			f.issuer.Claims = map[string]any{"preferred_username": "mallory"}
			flow, callback := f.startSignIn(t, "boxbox_oidc_flow")
			return f.do(t, http.MethodGet, callback, flow)
		}},
		{name: "provider down", want: "provider_unavailable", run: func(t *testing.T, f oidcFixture) *httptest.ResponseRecorder {
			f.issuer.FailDiscovery = true
			return f.do(t, http.MethodGet, "/api/v1/auth/oidc/login")
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			f := newOIDCFixture(t, "http://boxbox.test/api/v1/auth/oidc/callback", nil)
			f.issuer.Claims = map[string]any{"preferred_username": "carol"}
			if code := ssoErrorParam(t, test.run(t, f)); code != test.want {
				t.Fatalf("sso_error = %q, want %q", code, test.want)
			}
		})
	}
}

func TestOIDCHTTPSRedirectUsesHostPrefixedSecureCookies(t *testing.T) {
	f := newOIDCFixture(t, "https://files.example.com/api/v1/auth/oidc/callback", nil)
	f.issuer.Claims = map[string]any{"preferred_username": "carol"}
	flow, callback := f.startSignIn(t, "__Host-boxbox_oidc_flow")
	if !flow.Secure || flow.Path != "/" || flow.Domain != "" {
		t.Fatalf("flow cookie = %#v", flow)
	}
	// TLS ends at the proxy, which did not send X-Forwarded-Proto.
	rec := f.do(t, http.MethodGet, callback, flow)
	if refresh := cookieNamed(rec, refreshCookieName); refresh == nil || !refresh.Secure {
		t.Fatalf("refresh cookie = %#v, want Secure", refresh)
	}
	rec = f.do(t, http.MethodPost, "http://boxbox.test/api/v1/auth/refresh", cookieNamed(rec, refreshCookieName))
	if refresh := cookieNamed(rec, refreshCookieName); rec.Code != http.StatusOK || refresh == nil || !refresh.Secure {
		t.Fatalf("refreshed cookie = %#v, status %d, want Secure", refresh, rec.Code)
	}
	rec = f.do(t, http.MethodPost, "http://boxbox.test/api/v1/auth/logout", cookieNamed(rec, refreshCookieName))
	if cleared := cookieNamed(rec, refreshCookieName); rec.Code != http.StatusOK || cleared == nil || !cleared.Secure || cleared.MaxAge >= 0 {
		t.Fatalf("logout cookie = %#v, status %d, want Secure deletion", cleared, rec.Code)
	}
	// A cookie planted by a sibling domain cannot use the __Host- name, so it is ignored.
	_, callback = f.startSignIn(t, "__Host-boxbox_oidc_flow")
	tossed := &http.Cookie{Name: "boxbox_oidc_flow", Value: flow.Value}
	if code := ssoErrorParam(t, f.do(t, http.MethodGet, callback, tossed)); code != "flow_expired" {
		t.Fatalf("tossed cookie sso_error = %q", code)
	}
}

func TestPasswordLoginDisabledReturns403(t *testing.T) {
	f := newOIDCFixture(t, "http://boxbox.test/api/v1/auth/oidc/callback", func(cfg *service.AuthServiceConfig) {
		cfg.PasswordLoginDisabled = true
	})
	req := httptest.NewRequest(http.MethodPost, "http://boxbox.test/api/v1/auth/login",
		strings.NewReader(`{"username":"admin","password":"correct-password"}`))
	rec := httptest.NewRecorder()
	f.router.ServeHTTP(rec, req)
	var body model.ErrorResponse
	if rec.Code != http.StatusForbidden || json.Unmarshal(rec.Body.Bytes(), &body) != nil || body.Code != model.ErrCodePasswordLoginDisabled {
		t.Fatalf("login = %d %s", rec.Code, rec.Body.String())
	}
}

func TestAuthConfigResponse(t *testing.T) {
	for _, test := range []struct {
		config AuthConfigResponse
		want   string
	}{
		{AuthConfigResponse{PasswordLogin: true}, `{"passwordLogin":true,"oidc":null}`},
		{AuthConfigResponse{PasswordLogin: false, OIDC: &AuthConfigOIDC{Name: "Authelia"}}, `{"passwordLogin":false,"oidc":{"name":"Authelia"}}`},
	} {
		rec := httptest.NewRecorder()
		AuthConfig(test.config)(rec, httptest.NewRequest(http.MethodGet, "/api/v1/auth/config", nil))
		if got := strings.TrimSpace(rec.Body.String()); got != test.want {
			t.Errorf("config = %s, want %s", got, test.want)
		}
	}
}
