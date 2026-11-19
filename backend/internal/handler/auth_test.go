package handler

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/middleware"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"github.com/spf13/afero"
	"golang.org/x/crypto/bcrypt"
)

func TestAuthRefreshTokenUsesHttpOnlyCookie(t *testing.T) {
	hash, err := bcrypt.GenerateFromPassword([]byte("correct-password"), bcrypt.MinCost)
	if err != nil {
		t.Fatal(err)
	}
	auth, err := service.NewAuthService(filesystem.NewOsFS(), service.AuthServiceConfig{
		DataDir:   t.TempDir(),
		JWTSecret: "0123456789abcdef0123456789abcdef",
		Users:     map[string]string{"admin": string(hash)},
	})
	if err != nil {
		t.Fatalf("create auth service: %v", err)
	}
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
	logoutResponse := httptest.NewRecorder()
	handler.Logout(logoutResponse, logoutRequest)
	if logoutResponse.Code != http.StatusOK {
		t.Fatalf("logout status = %d body=%s", logoutResponse.Code, logoutResponse.Body.String())
	}
	clearedCookie := onlyCookie(t, logoutResponse)
	if clearedCookie.Value != "" || clearedCookie.MaxAge >= 0 {
		t.Fatalf("successful logout cookie = %#v, want deletion", clearedCookie)
	}
}
func TestAuthLogoutRevokesEveryAccessTokenForOnlyThatSession(t *testing.T) {
	dataDir := t.TempDir()
	auth, handler := newAuthHandler(t, filesystem.NewOsFS(), dataDir)
	login := func() (LoginResponse, *http.Cookie) {
		t.Helper()
		request := httptest.NewRequest(http.MethodPost, "http://boxbox.example/api/v1/auth/login",
			bytes.NewBufferString(`{"username":"admin","password":"correct-password"}`))
		response := httptest.NewRecorder()
		handler.Login(response, request)
		if response.Code != http.StatusOK {
			t.Fatalf("login status = %d; body=%s", response.Code, response.Body.String())
		}
		var pair LoginResponse
		if err := json.Unmarshal(response.Body.Bytes(), &pair); err != nil {
			t.Fatal(err)
		}
		return pair, onlyCookie(t, response)
	}

	firstPair, firstCookie := login()
	independentPair, _ := login()
	rotated := refreshRequest(handler, firstCookie)
	if rotated.Code != http.StatusOK {
		t.Fatalf("refresh status = %d; body=%s", rotated.Code, rotated.Body.String())
	}
	var rotatedPair LoginResponse
	if err := json.Unmarshal(rotated.Body.Bytes(), &rotatedPair); err != nil {
		t.Fatal(err)
	}
	rotatedCookie := onlyCookie(t, rotated)

	logoutRequest := httptest.NewRequest(http.MethodPost, "http://boxbox.example/api/v1/auth/logout", nil)
	logoutRequest.AddCookie(rotatedCookie)
	logoutResponse := httptest.NewRecorder()
	handler.Logout(logoutResponse, logoutRequest)
	if logoutResponse.Code != http.StatusOK {
		t.Fatalf("logout status = %d; body=%s", logoutResponse.Code, logoutResponse.Body.String())
	}

	protected := middleware.JWTAuth(auth)(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	for _, token := range []struct {
		name       string
		access     string
		wantStatus int
	}{
		{name: "access issued before rotation", access: firstPair.AccessToken, wantStatus: http.StatusUnauthorized},
		{name: "access issued by rotation", access: rotatedPair.AccessToken, wantStatus: http.StatusUnauthorized},
		{name: "independent login", access: independentPair.AccessToken, wantStatus: http.StatusOK},
	} {
		t.Run(token.name, func(t *testing.T) {
			request := httptest.NewRequest(http.MethodGet, "/api/v1/files", nil)
			request.Header.Set("Authorization", "Bearer "+token.access)
			response := httptest.NewRecorder()
			protected.ServeHTTP(response, request)
			if response.Code != token.wantStatus {
				t.Fatalf("protected request status = %d, want %d", response.Code, token.wantStatus)
			}
		})
	}
}

func TestAuthLoginPersistenceFailuresDoNotIssueCredentials(t *testing.T) {
	loginRequest := func(cookie *http.Cookie) *http.Request {
		request := httptest.NewRequest(http.MethodPost, "http://boxbox.example/api/v1/auth/login",
			bytes.NewBufferString(`{"username":"admin","password":"correct-password"}`))
		if cookie != nil {
			request.AddCookie(cookie)
		}
		return request
	}

	t.Run("pre-publication failure preserves unrelated cookie and retry works", func(t *testing.T) {
		dataDir := t.TempDir()
		fsys := &publicationFailFS{FS: filesystem.NewOsFS(), dataDir: dataDir}
		_, handler := newAuthHandler(t, fsys, dataDir)
		existingCookie := loginCookie(t, handler)
		fsys.failure = failBeforePublish

		failed := httptest.NewRecorder()
		handler.Login(failed, loginRequest(existingCookie))
		if failed.Code != http.StatusInternalServerError {
			t.Fatalf("login status = %d, want 500; body=%s", failed.Code, failed.Body.String())
		}
		if cookies := failed.Result().Cookies(); len(cookies) != 0 {
			t.Fatalf("failed login Set-Cookie = %#v, want none", cookies)
		}
		if strings.Contains(failed.Body.String(), "accessToken") ||
			strings.Contains(failed.Body.String(), "refreshToken") {
			t.Fatal("failed login exposed credentials")
		}

		fsys.failure = failNever
		retried := httptest.NewRecorder()
		handler.Login(retried, loginRequest(existingCookie))
		if retried.Code != http.StatusOK {
			t.Fatalf("login retry status = %d; body=%s", retried.Code, retried.Body.String())
		}
		onlyCookie(t, retried)
		fsys.failure = failNever
		existingSession := refreshRequest(handler, existingCookie)
		if existingSession.Code != http.StatusOK {
			t.Fatalf("existing session refresh status = %d; body=%s", existingSession.Code, existingSession.Body.String())
		}
	})

	t.Run("post-publication failure requires recovery without a cookie", func(t *testing.T) {
		dataDir := t.TempDir()
		fsys := &publicationFailFS{FS: filesystem.NewOsFS(), dataDir: dataDir}
		_, handler := newAuthHandler(t, fsys, dataDir)
		existingCookie := loginCookie(t, handler)
		fsys.failure = failAfterPublish

		failed := httptest.NewRecorder()
		handler.Login(failed, loginRequest(existingCookie))
		if failed.Code != http.StatusUnauthorized {
			t.Fatalf("login status = %d, want 401; body=%s", failed.Code, failed.Body.String())
		}
		var body model.ErrorResponse
		if err := json.Unmarshal(failed.Body.Bytes(), &body); err != nil {
			t.Fatal(err)
		}
		if body.Code != model.ErrCodeSessionRecoveryRequired {
			t.Fatalf("login recovery error = %#v", body)
		}
		if cookies := failed.Result().Cookies(); len(cookies) != 0 {
			t.Fatalf("recovery login Set-Cookie = %#v, want none", cookies)
		}
		if strings.Contains(failed.Body.String(), "accessToken") ||
			strings.Contains(failed.Body.String(), "refreshToken") {
			t.Fatal("recovery login exposed credentials")
		}
		fsys.failure = failNever
		existingSession := refreshRequest(handler, existingCookie)
		if existingSession.Code != http.StatusOK {
			t.Fatalf("existing session refresh status = %d; body=%s", existingSession.Code, existingSession.Body.String())
		}
	})
}

func TestAuthPersistenceFailuresPreserveOrClearCookieByOutcome(t *testing.T) {
	t.Run("pre-publication refresh failure preserves cookie for retry", func(t *testing.T) {
		dataDir := t.TempDir()
		fsys := &publicationFailFS{FS: filesystem.NewOsFS(), dataDir: dataDir}
		auth, handler := newAuthHandler(t, fsys, dataDir)

		cookie := loginCookie(t, handler)

		fsys.failure = failBeforePublish
		failed := refreshRequest(handler, cookie)
		if failed.Code != http.StatusInternalServerError {
			t.Fatalf("refresh status = %d, want 500; body=%s", failed.Code, failed.Body.String())
		}
		if got := failed.Result().Cookies(); len(got) != 0 {
			t.Fatalf("ordinary failure Set-Cookie = %#v, want none", got)
		}

		fsys.failure = failNever
		retried := refreshRequest(handler, cookie)
		if retried.Code != http.StatusOK {
			t.Fatalf("retry status = %d, want 200; body=%s", retried.Code, retried.Body.String())
		}
		rotated := onlyCookie(t, retried)
		if rotated.Value == cookie.Value {
			t.Fatal("successful retry did not rotate refresh cookie")
		}
		if _, err := auth.ValidateRefreshToken(cookie.Value); !errors.Is(err, service.ErrRefreshSuperseded) {
			t.Fatalf("previous refresh token after retry = %v, want superseded", err)
		}
	})

	t.Run("post-publication failure is terminal and keeps the published rotation", func(t *testing.T) {
		dataDir := t.TempDir()
		fsys := &publicationFailFS{FS: filesystem.NewOsFS(), dataDir: dataDir}
		_, handler := newAuthHandler(t, fsys, dataDir)
		cookie := loginCookie(t, handler)

		fsys.failure = failAfterPublish
		failed := refreshRequest(handler, cookie)
		if failed.Code != http.StatusUnauthorized {
			t.Fatalf("refresh status = %d, want 401; body=%s", failed.Code, failed.Body.String())
		}
		var body model.ErrorResponse
		if err := json.Unmarshal(failed.Body.Bytes(), &body); err != nil {
			t.Fatal(err)
		}
		if body.Code != model.ErrCodeSessionRecoveryRequired ||
			!strings.Contains(strings.ToLower(body.Error), "sign in") {
			t.Fatalf("recovery error response = %#v", body)
		}
		cleared := onlyCookie(t, failed)
		if cleared.Value != "" || cleared.MaxAge >= 0 {
			t.Fatalf("terminal cookie = %#v, want deletion", cleared)
		}
		if strings.Contains(failed.Body.String(), cookie.Value) ||
			strings.Contains(failed.Body.String(), "accessToken") {
			t.Fatal("terminal response exposed credentials")
		}

		restarted, err := service.NewAuthService(filesystem.NewOsFS(), authConfig(t, dataDir))
		if err != nil {
			t.Fatalf("restart auth service: %v", err)
		}
		if _, err := restarted.ValidateRefreshToken(cookie.Value); !errors.Is(err, service.ErrRefreshSuperseded) {
			t.Fatalf("published refresh rotation after restart = %v, want superseded", err)
		}
	})

	t.Run("ordinary logout failure retains cookie", func(t *testing.T) {
		dataDir := t.TempDir()
		fsys := &publicationFailFS{FS: filesystem.NewOsFS(), dataDir: dataDir}
		_, handler := newAuthHandler(t, fsys, dataDir)
		cookie := loginCookie(t, handler)
		fsys.failure = failBeforePublish

		request := httptest.NewRequest(http.MethodPost, "http://boxbox.example/api/v1/auth/logout", nil)
		request.AddCookie(cookie)
		response := httptest.NewRecorder()
		handler.Logout(response, request)
		if response.Code != http.StatusInternalServerError {
			t.Fatalf("logout status = %d, want 500; body=%s", response.Code, response.Body.String())
		}
		if got := response.Result().Cookies(); len(got) != 0 {
			t.Fatalf("ordinary logout failure Set-Cookie = %#v, want none", got)
		}
	})

	t.Run("published logout failure is terminal, not success", func(t *testing.T) {
		dataDir := t.TempDir()
		fsys := &publicationFailFS{FS: filesystem.NewOsFS(), dataDir: dataDir}
		_, handler := newAuthHandler(t, fsys, dataDir)
		cookie := loginCookie(t, handler)
		fsys.failure = failAfterPublish

		request := httptest.NewRequest(http.MethodPost, "http://boxbox.example/api/v1/auth/logout", nil)
		request.AddCookie(cookie)
		response := httptest.NewRecorder()
		handler.Logout(response, request)
		if response.Code != http.StatusUnauthorized {
			t.Fatalf("logout status = %d, want 401; body=%s", response.Code, response.Body.String())
		}
		var body model.ErrorResponse
		if err := json.Unmarshal(response.Body.Bytes(), &body); err != nil {
			t.Fatal(err)
		}
		if body.Code != model.ErrCodeSessionRecoveryRequired ||
			strings.Contains(response.Body.String(), "successfully") {
			t.Fatalf("logout recovery response = %#v", body)
		}
		cleared := onlyCookie(t, response)
		if cleared.Value != "" || cleared.MaxAge >= 0 {
			t.Fatalf("terminal logout cookie = %#v, want deletion", cleared)
		}
	})
}

func TestAuthCookieSecurityAndOriginBoundaries(t *testing.T) {
	handler := NewAuthHandler(nil)
	for _, testCase := range []struct {
		name       string
		request    *http.Request
		wantSecure bool
	}{
		{name: "plain HTTP", request: httptest.NewRequest(http.MethodPost, "http://boxbox.example/api/v1/auth/login", nil)},
		{name: "TLS", request: httptest.NewRequest(http.MethodPost, "https://boxbox.example/api/v1/auth/login", nil), wantSecure: true},
		{
			name: "forwarded HTTPS",
			request: func() *http.Request {
				r := httptest.NewRequest(http.MethodPost, "http://boxbox.example/api/v1/auth/login", nil)
				r.Header.Set("X-Forwarded-Proto", "HTTPS")
				return r
			}(),
			wantSecure: true,
		},
	} {
		t.Run(testCase.name, func(t *testing.T) {
			recorder := httptest.NewRecorder()
			handler.setRefreshCookie(recorder, testCase.request, "opaque-test-value", time.Now().Add(time.Hour))
			cookie := onlyCookie(t, recorder)
			if cookie.Secure != testCase.wantSecure || !cookie.HttpOnly || cookie.SameSite != http.SameSiteStrictMode {
				t.Fatalf("cookie attributes = %#v", cookie)
			}
		})
	}

	for _, endpoint := range []string{"login", "refresh", "logout"} {
		t.Run("cross-origin "+endpoint, func(t *testing.T) {
			request := httptest.NewRequest(http.MethodPost, "https://boxbox.example/api/v1/auth/"+endpoint, nil)
			request.Header.Set("Origin", "https://evil.example")
			response := httptest.NewRecorder()
			switch endpoint {
			case "login":
				handler.Login(response, request)
			case "refresh":
				handler.Refresh(response, request)
			case "logout":
				handler.Logout(response, request)
			}
			if response.Code != http.StatusForbidden {
				t.Fatalf("status = %d, want 403", response.Code)
			}
			if cookies := response.Result().Cookies(); len(cookies) != 0 {
				t.Fatalf("cross-origin Set-Cookie = %#v, want none", cookies)
			}
		})
	}
}

func TestAuthRefreshInvalidTokenClearsCookie(t *testing.T) {
	_, handler := newAuthHandler(t, filesystem.NewOsFS(), t.TempDir())
	request := httptest.NewRequest(http.MethodPost, "http://boxbox.example/api/v1/auth/refresh", nil)
	request.AddCookie(&http.Cookie{Name: refreshCookieName, Value: "malformed"})
	response := httptest.NewRecorder()
	handler.Refresh(response, request)
	if response.Code != http.StatusUnauthorized {
		t.Fatalf("refresh status = %d, want 401", response.Code)
	}
	var body model.ErrorResponse
	if err := json.Unmarshal(response.Body.Bytes(), &body); err != nil {
		t.Fatal(err)
	}
	if body.Code != model.ErrCodeTokenInvalid {
		t.Fatalf("error code = %q, want %q", body.Code, model.ErrCodeTokenInvalid)
	}
	cookie := onlyCookie(t, response)
	if cookie.Value != "" || cookie.MaxAge >= 0 {
		t.Fatalf("invalid-token cookie = %#v, want deletion", cookie)
	}
}

type persistenceFailure int

const (
	failNever persistenceFailure = iota
	failBeforePublish
	failAfterPublish
)

type publicationFailFS struct {
	filesystem.FS
	dataDir   string
	failure   persistenceFailure
	published bool
}

func (f *publicationFailFS) Rename(oldPath, newPath string) error {
	if filepath.Base(newPath) != config.AuthSessionsFileName {
		return f.FS.Rename(oldPath, newPath)
	}
	if f.failure == failBeforePublish {
		return errors.New("injected pre-publication failure")
	}
	if err := f.FS.Rename(oldPath, newPath); err != nil {
		return err
	}
	f.published = f.failure == failAfterPublish
	return nil
}

func (f *publicationFailFS) Open(name string) (afero.File, error) {
	file, err := f.FS.Open(name)
	if err != nil {
		return nil, err
	}
	if f.published && name == f.dataDir {
		return &syncFailureFile{File: file}, nil
	}
	return file, nil
}

type syncFailureFile struct {
	afero.File
}

func (f *syncFailureFile) Sync() error {
	return errors.New("injected post-publication directory sync failure")
}

func newAuthHandler(t *testing.T, fsys filesystem.FS, dataDir string) (service.AuthService, *AuthHandler) {
	t.Helper()
	auth, err := service.NewAuthService(fsys, authConfig(t, dataDir))
	if err != nil {
		t.Fatalf("create auth service: %v", err)
	}
	return auth, NewAuthHandler(auth)
}

func authConfig(t *testing.T, dataDir string) service.AuthServiceConfig {
	t.Helper()
	hash, err := bcrypt.GenerateFromPassword([]byte("correct-password"), bcrypt.MinCost)
	if err != nil {
		t.Fatal(err)
	}
	return service.AuthServiceConfig{
		DataDir:   dataDir,
		JWTSecret: "0123456789abcdef0123456789abcdef",
		Users:     map[string]string{"admin": string(hash)},
	}
}

func loginCookie(t *testing.T, handler *AuthHandler) *http.Cookie {
	t.Helper()
	request := httptest.NewRequest(http.MethodPost, "http://boxbox.example/api/v1/auth/login",
		bytes.NewBufferString(`{"username":"admin","password":"correct-password"}`))
	response := httptest.NewRecorder()
	handler.Login(response, request)
	if response.Code != http.StatusOK {
		t.Fatalf("login status = %d; body=%s", response.Code, response.Body.String())
	}
	return onlyCookie(t, response)
}

func refreshRequest(handler *AuthHandler, cookie *http.Cookie) *httptest.ResponseRecorder {
	request := httptest.NewRequest(http.MethodPost, "http://boxbox.example/api/v1/auth/refresh", nil)
	request.AddCookie(cookie)
	response := httptest.NewRecorder()
	handler.Refresh(response, request)
	return response
}

func onlyCookie(t *testing.T, response *httptest.ResponseRecorder) *http.Cookie {
	t.Helper()
	cookies := response.Result().Cookies()
	if len(cookies) != 1 {
		t.Fatalf("cookie count = %d, want 1", len(cookies))
	}
	return cookies[0]
}

func TestSupersededRefreshAndLogoutPreserveSharedCookie(t *testing.T) {
	auth, handler := newAuthHandler(t, filesystem.NewOsFS(), t.TempDir())
	original, err := auth.Login(context.Background(), "admin", "correct-password")
	if err != nil {
		t.Fatal(err)
	}
	replacement, err := auth.Refresh(context.Background(), original.RefreshToken)
	if err != nil {
		t.Fatal(err)
	}
	for _, action := range []struct {
		name  string
		serve http.HandlerFunc
	}{
		{"refresh", handler.Refresh}, {"logout", handler.Logout},
	} {
		t.Run(action.name, func(t *testing.T) {
			request := httptest.NewRequest(http.MethodPost, "http://boxbox.example/api/v1/auth/"+action.name, nil)
			request.AddCookie(&http.Cookie{Name: refreshCookieName, Value: original.RefreshToken})
			response := httptest.NewRecorder()
			action.serve(response, request)
			if response.Code != http.StatusConflict {
				t.Fatalf("status = %d, want 409", response.Code)
			}
			if response.Header().Get("Set-Cookie") != "" {
				t.Fatal("stale request overwrote the shared cookie")
			}
			var body model.ErrorResponse
			if err := json.Unmarshal(response.Body.Bytes(), &body); err != nil {
				t.Fatal(err)
			}
			if body.Code != model.ErrCodeRefreshSuperseded {
				t.Fatalf("code = %s", body.Code)
			}
			if _, err := auth.ValidateRefreshToken(replacement.RefreshToken); err != nil {
				t.Fatal(err)
			}
		})
	}
}
