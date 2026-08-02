package handler

import (
	"net/http/httptest"
	"testing"
)

func TestWebSocketOriginDefaultsToSameOrigin(t *testing.T) {
	handler := NewWebSocketHandler(nil, nil, nil)

	sameOrigin := httptest.NewRequest("GET", "http://boxbox.example/api/v1/ws", nil)
	sameOrigin.Host = "boxbox.example"
	sameOrigin.Header.Set("Origin", "http://boxbox.example")
	if !handler.checkOrigin(sameOrigin) {
		t.Fatal("same-origin WebSocket was rejected")
	}

	crossOrigin := httptest.NewRequest("GET", "http://boxbox.example/api/v1/ws", nil)
	crossOrigin.Host = "boxbox.example"
	crossOrigin.Header.Set("Origin", "https://evil.example")
	if handler.checkOrigin(crossOrigin) {
		t.Fatal("cross-origin WebSocket was accepted without an allow-list")
	}
}

func TestWebSocketOriginWildcardIsExplicit(t *testing.T) {
	handler := NewWebSocketHandler(nil, nil, []string{"*"})
	request := httptest.NewRequest("GET", "http://boxbox.example/api/v1/ws", nil)
	request.Header.Set("Origin", "https://other.example")
	if !handler.checkOrigin(request) {
		t.Fatal("explicit wildcard did not allow origin")
	}
}
