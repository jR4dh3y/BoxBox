package middleware

import (
	"net/http/httptest"
	"testing"
)

func TestGetClientIPIgnoresForwardedHeaders(t *testing.T) {
	request := httptest.NewRequest("POST", "/api/v1/auth/login", nil)
	request.RemoteAddr = "192.0.2.10:4321"
	request.Header.Set("X-Forwarded-For", "203.0.113.99")
	request.Header.Set("X-Real-IP", "203.0.113.100")

	if got := getClientIP(request); got != "192.0.2.10" {
		t.Fatalf("getClientIP() = %q, want socket address", got)
	}
}

func TestGetClientIPHandlesIPv6RemoteAddr(t *testing.T) {
	request := httptest.NewRequest("POST", "/api/v1/auth/login", nil)
	request.RemoteAddr = "[2001:db8::1]:4321"

	if got := getClientIP(request); got != "2001:db8::1" {
		t.Fatalf("getClientIP() = %q, want IPv6 host", got)
	}
}

func TestTrustedProxyUsesForwardedClientIP(t *testing.T) {
	request := httptest.NewRequest("POST", "/api/v1/auth/login", nil)
	request.RemoteAddr = "10.0.0.4:4321"
	request.Header.Set("X-Forwarded-For", "203.0.113.99, 10.0.0.3")

	trusted := parseTrustedProxies([]string{"10.0.0.0/8"})
	if got := getClientIPFromTrustedProxies(request, trusted); got != "203.0.113.99" {
		t.Fatalf("client IP = %q, want first untrusted hop", got)
	}
}

func TestUntrustedPeerCannotSpoofForwardedClientIP(t *testing.T) {
	request := httptest.NewRequest("POST", "/api/v1/auth/login", nil)
	request.RemoteAddr = "192.0.2.10:4321"
	request.Header.Set("X-Forwarded-For", "203.0.113.99")

	trusted := parseTrustedProxies([]string{"10.0.0.0/8"})
	if got := getClientIPFromTrustedProxies(request, trusted); got != "192.0.2.10" {
		t.Fatalf("client IP = %q, want socket peer", got)
	}
}
