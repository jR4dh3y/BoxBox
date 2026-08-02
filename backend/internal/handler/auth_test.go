package handler

import (
	"bytes"
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"golang.org/x/crypto/bcrypt"
)

func TestAuthRefreshTokenUsesHttpOnlyCookie(t *testing.T) {
	hash, err := bcrypt.GenerateFromPassword([]byte("correct-password"), bcrypt.MinCost)
	if err != nil {
		t.Fatal(err)
	}
	auth := service.NewAuthService(service.AuthServiceConfig{
		JWTSecret: "0123456789abcdef0123456789abcdef",
		Users:     map[string]string{"admin": string(hash)},
	})
	handler := NewAuthHandler(auth)

	loginRequest := httptest.NewRequest(
		http.MethodPost,
		"http://boxbox.example/api/v1/auth/login",
		bytes.NewBufferString(`{"username":"admin","password":"correct-password"}`),
	)
	loginResponse := httptest.NewRecorder()
	handler.Login(loginResponse, loginRequest)
	if loginResponse.Code != http.StatusOK {
		t.Fatalf("login status = %d body=%s", loginResponse.Code, loginResponse.Body.String())
	}
	var body map[string]any
	if err := json.Unmarshal(loginResponse.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if _, exposed := body["refreshToken"]; exposed {
		t.Fatal("refresh token was exposed in JSON")
	}
	accessToken, _ := body["accessToken"].(string)
	if accessToken == "" {
		t.Fatal("missing access token")
	}

	result := loginResponse.Result()
	cookies := result.Cookies()
	if len(cookies) != 1 {
		t.Fatalf("cookie count = %d, want 1", len(cookies))
	}
	refreshCookie := cookies[0]
	if !refreshCookie.HttpOnly || refreshCookie.SameSite != http.SameSiteStrictMode ||
		refreshCookie.Path != "/api/v1/auth" {
		t.Fatalf("insecure refresh cookie attributes: %#v", refreshCookie)
	}
	if strings.Contains(loginResponse.Body.String(), refreshCookie.Value) {
		t.Fatal("refresh token cookie value leaked into response body")
	}

	refreshRequest := httptest.NewRequest(
		http.MethodPost,
		"http://boxbox.example/api/v1/auth/refresh",
		nil,
	)
	refreshRequest.AddCookie(refreshCookie)
	refreshResponse := httptest.NewRecorder()
	handler.Refresh(refreshResponse, refreshRequest)
	if refreshResponse.Code != http.StatusOK {
		t.Fatalf("refresh status = %d body=%s", refreshResponse.Code, refreshResponse.Body.String())
	}

	rotatedCookie := refreshResponse.Result().Cookies()[0]
	logoutRequest := httptest.NewRequest(
		http.MethodPost,
		"http://boxbox.example/api/v1/auth/logout",
		nil,
	).WithContext(context.Background())
	logoutRequest.AddCookie(rotatedCookie)
	logoutRequest.Header.Set("Authorization", "Bearer "+accessToken)
	logoutResponse := httptest.NewRecorder()
	handler.Logout(logoutResponse, logoutRequest)
	if logoutResponse.Code != http.StatusOK {
		t.Fatalf("logout status = %d body=%s", logoutResponse.Code, logoutResponse.Body.String())
	}
}

func TestAuthRejectsCrossOriginCookieRequests(t *testing.T) {
	handler := NewAuthHandler(nil)
	request := httptest.NewRequest(http.MethodPost, "https://boxbox.example/api/v1/auth/refresh", nil)
	request.Header.Set("Origin", "https://evil.example")
	response := httptest.NewRecorder()
	handler.Refresh(response, request)
	if response.Code != http.StatusForbidden {
		t.Fatalf("status = %d, want forbidden", response.Code)
	}
}
