package handler

import (
	"encoding/json"
	"fmt"
	"net/http/httptest"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func TestConnectErrorMappings(t *testing.T) {
	for _, test := range []struct {
		name   string
		err    error
		status int
		code   string
	}{
		{"pairing", service.ErrPairingUnavailable, 404, "PAIRING_UNAVAILABLE"},
		{"rate", service.ErrConnectRateLimited, 429, "RATE_LIMITED"},
		{"in use", service.ErrPairingInUse, 409, model.ErrCodeConflict},
		{"name", service.ErrInvalidClientName, 400, model.ErrCodeValidationError},
		{"client", service.ErrConnectClientNotFound, 404, model.ErrCodeNotFound},
		{"limit", service.ErrConnectClientLimit, 409, model.ErrCodeConflict},
		{"storage", service.ErrConnectStorage, 503, model.ErrCodeInternalError},
	} {
		t.Run(test.name, func(t *testing.T) {
			rec := httptest.NewRecorder()
			HandleServiceError(rec, fmt.Errorf("private implementation detail: %w", test.err))
			var body model.ErrorResponse
			if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
				t.Fatal(err)
			}
			if rec.Code != test.status || body.Code != test.code {
				t.Fatalf("status/code = %d/%s, want %d/%s", rec.Code, body.Code, test.status, test.code)
			}
		})
	}
}
