package handler

import (
	"context"
	"net/http"
	"net/url"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"github.com/rs/zerolog/log"
)

const (
	// The __Host- prefix stops a sibling subdomain from planting a flow cookie (login CSRF).
	oidcFlowCookieSecure   = "__Host-boxbox_oidc_flow"
	oidcFlowCookieInsecure = "boxbox_oidc_flow"
	oidcFlowCookieMaxAge   = 10 * time.Minute
	ssoSuccessPath         = "/browse"
)

// SSOFlow runs the identity provider protocol.
type SSOFlow interface {
	Begin(ctx context.Context) (authURL, flowToken string, err error)
	Complete(ctx context.Context, flowToken string, callback service.OIDCCallback) (string, error)
}

// OIDCHandler serves the browser side of single sign-on.
type OIDCHandler struct {
	flow   SSOFlow
	auth   service.AuthService
	secure bool // the configured redirect URL is https
}

// NewOIDCHandler builds the handler. redirectURL decides whether cookies are Secure,
// so a proxy that drops X-Forwarded-Proto cannot downgrade them.
func NewOIDCHandler(flow SSOFlow, auth service.AuthService, redirectURL string) *OIDCHandler {
	parsed, err := url.Parse(redirectURL)
	return &OIDCHandler{flow: flow, auth: auth, secure: err == nil && parsed.Scheme == "https"}
}

// RegisterRoutes registers the sign-in routes inside the auth group.
func (h *OIDCHandler) RegisterRoutes(r chi.Router) {
	r.Get("/oidc/login", h.Login)
	r.Get("/oidc/callback", h.Callback)
}

// Login sends the browser to the identity provider.
// GET /api/v1/auth/oidc/login
func (h *OIDCHandler) Login(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Cache-Control", "no-store")
	authURL, flowToken, err := h.flow.Begin(r.Context())
	if err != nil {
		h.fail(w, r, err)
		return
	}
	http.SetCookie(w, h.flowCookie(flowToken, int(oidcFlowCookieMaxAge.Seconds())))
	http.Redirect(w, r, authURL, http.StatusFound)
}

// Callback finishes sign-in and lands on the app, which restores the session from the refresh cookie.
// GET /api/v1/auth/oidc/callback
func (h *OIDCHandler) Callback(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("Referrer-Policy", "no-referrer")
	http.SetCookie(w, h.flowCookie("", -1))

	cookie, err := r.Cookie(h.flowCookieName())
	if err != nil || cookie.Value == "" {
		h.fail(w, r, service.ErrOIDCFlow)
		return
	}
	query := r.URL.Query()
	username, err := h.flow.Complete(r.Context(), cookie.Value, service.OIDCCallback{
		State: query.Get("state"),
		Code:  query.Get("code"),
		Error: query.Get("error"),
	})
	if err != nil {
		h.fail(w, r, err)
		return
	}
	pair, err := h.auth.LoginSSO(r.Context(), username)
	if err != nil {
		if code := ssoErrorCode(err); code == "not_allowed" {
			log.Warn().Str("username", username).Msg("SSO sign-in refused: add the name to users or oidc.users")
		}
		h.fail(w, r, err)
		return
	}
	setRefreshCookie(w, pair.RefreshToken, pair.RefreshExpiresAt, h.secure || requestIsHTTPS(r))
	http.Redirect(w, r, ssoSuccessPath, http.StatusSeeOther)
}

// fail redirects to the login page with a fixed code; no request value reaches Location.
func (h *OIDCHandler) fail(w http.ResponseWriter, r *http.Request, err error) {
	code := ssoErrorCode(err)
	if code == "server_error" {
		log.Error().Err(err).Msg("SSO sign-in failed")
	}
	http.Redirect(w, r, "/login?sso_error="+code, http.StatusSeeOther)
}

func (h *OIDCHandler) flowCookieName() string {
	if h.secure {
		return oidcFlowCookieSecure
	}
	return oidcFlowCookieInsecure
}

func (h *OIDCHandler) flowCookie(value string, maxAge int) *http.Cookie {
	cookie := &http.Cookie{
		Name:     h.flowCookieName(),
		Value:    value,
		Path:     "/",
		MaxAge:   maxAge,
		HttpOnly: true,
		Secure:   h.secure,
		// Lax: the callback is a cross-site top-level navigation from the provider.
		SameSite: http.SameSiteLaxMode,
	}
	if !h.secure {
		cookie.Path = "/api/v1/auth/oidc"
	}
	return cookie
}

// AuthConfigResponse tells the login page which sign-in methods to show.
type AuthConfigResponse struct {
	PasswordLogin bool            `json:"passwordLogin"`
	OIDC          *AuthConfigOIDC `json:"oidc"`
}

// AuthConfigOIDC describes the single sign-on button.
type AuthConfigOIDC struct {
	Name string `json:"name"`
}

// AuthConfig serves the constant sign-in options.
// GET /api/v1/auth/config
func AuthConfig(config AuthConfigResponse) http.HandlerFunc {
	return func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Cache-Control", "no-cache")
		writeJSON(w, config, http.StatusOK)
	}
}
