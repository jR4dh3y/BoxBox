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
