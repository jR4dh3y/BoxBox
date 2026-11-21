package model

import (
	"fmt"
	"net/url"
	"slices"
	"strings"
	"unicode"
	"unicode/utf8"
)

// OIDCCallbackPath is the only callback path BoxBox serves; BoxBox runs at the root of its origin.
const OIDCCallbackPath = "/api/v1/auth/oidc/callback"

// OIDCConfig configures single sign-on through an OpenID Connect provider. It is off when IssuerURL is empty.
type OIDCConfig struct {
	IssuerURL            string   `mapstructure:"issuer_url"`
	ClientID             string   `mapstructure:"client_id"`
	ClientSecret         string   `mapstructure:"client_secret"`
	RedirectURL          string   `mapstructure:"redirect_url"`
	ProviderName         string   `mapstructure:"provider_name"`
	UsernameClaim        string   `mapstructure:"username_claim"`
	Scopes               []string `mapstructure:"scopes"`
	Users                []string `mapstructure:"users"` // accounts that sign in only through the provider
	DisablePasswordLogin bool     `mapstructure:"disable_password_login"`
}

// Enabled reports whether single sign-on is configured.
func (c OIDCConfig) Enabled() bool { return c.IssuerURL != "" }

// ValidUsername reports whether a name can be a BoxBox account: 1-64 bytes of
// valid UTF-8, no control characters and no surrounding whitespace.
func ValidUsername(name string) bool {
	if name == "" || len(name) > 64 || !utf8.ValidString(name) || strings.TrimSpace(name) != name {
		return false
	}
	return !strings.ContainsFunc(name, unicode.IsControl)
}

func (c OIDCConfig) validate() error {
	if !c.Enabled() {
		if c.ClientID != "" || c.ClientSecret != "" || c.RedirectURL != "" || len(c.Users) > 0 || c.DisablePasswordLogin {
			return fmt.Errorf("oidc.issuer_url is required when other oidc settings are set")
		}
		return nil
	}
	if !absoluteHTTPURL(c.IssuerURL) {
		return fmt.Errorf("oidc.issuer_url must be an absolute http(s) URL without query or fragment")
	}
	if c.ClientID == "" {
		return fmt.Errorf("oidc.client_id is required when oidc.issuer_url is set")
	}
	if c.RedirectURL == "" {
		return fmt.Errorf("oidc.redirect_url is required when oidc.issuer_url is set")
	}
	if redirect, err := url.Parse(c.RedirectURL); err != nil || !absoluteHTTPURL(c.RedirectURL) || redirect.Path != OIDCCallbackPath {
		return fmt.Errorf("oidc.redirect_url must be an absolute http(s) URL whose path is %s", OIDCCallbackPath)
	}
	if !slices.Contains(c.Scopes, "openid") {
		return fmt.Errorf("oidc.scopes must include \"openid\"")
	}
	if strings.TrimSpace(c.UsernameClaim) == "" {
		return fmt.Errorf("oidc.username_claim cannot be empty")
	}
	if name := c.ProviderName; name == "" || utf8.RuneCountInString(name) > 40 || strings.ContainsFunc(name, unicode.IsControl) {
		return fmt.Errorf("oidc.provider_name must be 1-40 printable characters")
	}
	for i, user := range c.Users {
		if !ValidUsername(user) {
			return fmt.Errorf("oidc.users[%d] is not a valid username", i)
		}
	}
	return nil
}

func absoluteHTTPURL(raw string) bool {
	u, err := url.Parse(raw)
	return err == nil && (u.Scheme == "http" || u.Scheme == "https") && u.Host != "" &&
		u.User == nil && u.RawQuery == "" && u.Fragment == ""
}
