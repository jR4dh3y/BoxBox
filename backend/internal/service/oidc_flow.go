package service

import (
	"crypto/hmac"
	"crypto/sha256"
	"time"

	"github.com/golang-jwt/jwt/v5"
)

const (
	oidcFlowIssuer   = "boxbox-oidc-flow"
	oidcFlowLifetime = 10 * time.Minute
)

// oidcFlow is the per-browser sign-in state. It travels in a signed cookie so
// the public start route keeps no server memory and the flow is bound to the
// browser that began it.
type oidcFlow struct {
	State    string `json:"state"`
	Nonce    string `json:"nonce"`
	Verifier string `json:"verifier"`
	jwt.RegisteredClaims
}

// oidcFlowKey derives a key distinct from the session key, so neither token type validates as the other.
func oidcFlowKey(jwtSecret string) []byte {
	mac := hmac.New(sha256.New, []byte(jwtSecret))
	mac.Write([]byte("boxbox oidc flow v1"))
	return mac.Sum(nil)
}

func (s *OIDCService) sealFlow(flow oidcFlow) (string, error) {
	now := time.Now()
	flow.RegisteredClaims = jwt.RegisteredClaims{
		Issuer:    oidcFlowIssuer,
		IssuedAt:  jwt.NewNumericDate(now),
		ExpiresAt: jwt.NewNumericDate(now.Add(oidcFlowLifetime)),
	}
	return jwt.NewWithClaims(jwt.SigningMethodHS256, &flow).SignedString(s.flowKey)
}

func (s *OIDCService) openFlow(token string) (oidcFlow, error) {
	var flow oidcFlow
	parsed, err := jwt.ParseWithClaims(token, &flow, func(*jwt.Token) (any, error) { return s.flowKey, nil },
		jwt.WithValidMethods([]string{jwt.SigningMethodHS256.Alg()}),
		jwt.WithIssuer(oidcFlowIssuer),
		jwt.WithExpirationRequired(),
	)
	if err != nil || !parsed.Valid || flow.State == "" || flow.Nonce == "" || flow.Verifier == "" {
		return oidcFlow{}, ErrOIDCFlow
	}
	return flow, nil
}
