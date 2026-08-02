package middleware

import (
	"bytes"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
)

func TestRequestLoggerOmitsQueryString(t *testing.T) {
	var output bytes.Buffer
	original := log.Logger
	log.Logger = zerolog.New(&output)
	t.Cleanup(func() { log.Logger = original })

	handler := RequestLogger(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Query().Get("width") != "224" {
			t.Error("logger removed query parameters from the handler request")
		}
		w.WriteHeader(http.StatusNoContent)
	}))
	request := httptest.NewRequest(
		http.MethodGet,
		"/api/v1/stream/thumbnail/home/photo.png?token=live-secret-token&width=224",
		nil,
	)
	handler.ServeHTTP(httptest.NewRecorder(), request)

	logged := output.String()
	if strings.Contains(logged, "live-secret-token") || strings.Contains(logged, "width=224") {
		t.Fatalf("query string leaked into log: %s", logged)
	}
	if !strings.Contains(logged, "/api/v1/stream/thumbnail/home/photo.png") {
		t.Fatalf("request path missing from log: %s", logged)
	}
}
