package middleware

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestRateLimitOrHandsExcessToFallback(t *testing.T) {
	next := http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) { w.WriteHeader(http.StatusOK) })
	fallback := http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) { w.WriteHeader(http.StatusAccepted) })
	handler := RateLimitOr(1, fallback)(next)

	codes := make([]int, 0, 3)
	for range 3 {
		rec := httptest.NewRecorder()
		req := httptest.NewRequest(http.MethodGet, "/s/token", nil)
		req.RemoteAddr = "192.0.2.1:1234"
		handler.ServeHTTP(rec, req)
		codes = append(codes, rec.Code)
	}
	// Burst is twice the rate.
	if codes[0] != http.StatusOK || codes[1] != http.StatusOK || codes[2] != http.StatusAccepted {
		t.Fatalf("codes = %v, want [200 200 202]", codes)
	}
}

func TestRateLimitStillAnswers429(t *testing.T) {
	handler := RateLimit(1)(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {}))
	var rec *httptest.ResponseRecorder
	for range 3 {
		rec = httptest.NewRecorder()
		req := httptest.NewRequest(http.MethodGet, "/", nil)
		req.RemoteAddr = "192.0.2.1:1234"
		handler.ServeHTTP(rec, req)
	}
	if rec.Code != http.StatusTooManyRequests || rec.Header().Get("Retry-After") != "1" {
		t.Fatalf("status = %d Retry-After = %q", rec.Code, rec.Header().Get("Retry-After"))
	}
}
