package handler

import (
	"bytes"
	"encoding/json"
	"math"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/go-chi/chi/v5"
)

func postShare(t *testing.T, router *chi.Mux, body map[string]any) *httptest.ResponseRecorder {
	t.Helper()
	encoded, err := json.Marshal(body)
	if err != nil {
		t.Fatal(err)
	}
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, newShareManagementRequest(http.MethodPost, "/api/v1/shares", bytes.NewReader(encoded)))
	return rec
}

func TestShareDeleteOnExpiryIsReportedToTheOwnerOnly(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)

	rec := postShare(t, router, map[string]any{"path": "media/file.txt", "expiresInSeconds": 3600, "deleteOnExpiry": true})
	if rec.Code != http.StatusCreated {
		t.Fatalf("create status = %d: %s", rec.Code, rec.Body.String())
	}
	var created struct {
		Token          string `json:"token"`
		DeleteOnExpiry bool   `json:"deleteOnExpiry"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &created); err != nil {
		t.Fatal(err)
	}
	if !created.DeleteOnExpiry {
		t.Fatalf("create response does not report deleteOnExpiry: %s", rec.Body.String())
	}

	rec = httptest.NewRecorder()
	router.ServeHTTP(rec, newShareManagementRequest(http.MethodGet, "/api/v1/shares", nil))
	var listed struct {
		Shares []struct {
			DeleteOnExpiry bool `json:"deleteOnExpiry"`
		} `json:"shares"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &listed); err != nil {
		t.Fatal(err)
	}
	if len(listed.Shares) != 1 || !listed.Shares[0].DeleteOnExpiry {
		t.Fatalf("list does not report deleteOnExpiry: %s", rec.Body.String())
	}

	rec = httptest.NewRecorder()
	router.ServeHTTP(rec, httptest.NewRequest(http.MethodGet, "/api/v1/share/"+created.Token, nil))
	if rec.Code != http.StatusOK || strings.Contains(rec.Body.String(), "deleteOnExpiry") {
		t.Fatalf("recipient info status = %d body = %s", rec.Code, rec.Body.String())
	}
}

func TestShareDeleteOnExpiryRejectsUnsafeRequests(t *testing.T) {
	tests := []struct {
		name       string
		body       map[string]any
		wantStatus int
	}{
		{name: "folder", body: map[string]any{"path": "media/shared", "expiresInSeconds": 3600, "deleteOnExpiry": true}, wantStatus: http.StatusBadRequest},
		{name: "no expiry", body: map[string]any{"path": "media/file.txt", "deleteOnExpiry": true}, wantStatus: http.StatusBadRequest},
		{name: "read-only mount", body: map[string]any{"path": "archive/file.txt", "expiresInSeconds": 3600, "deleteOnExpiry": true}, wantStatus: http.StatusForbidden},
		// Without the bound this overflows time.Duration into a past expiry.
		{name: "overflowing expiry", body: map[string]any{"path": "media/file.txt", "expiresInSeconds": int64(math.MaxInt64 / 1_000_000_000 * 2), "deleteOnExpiry": true}, wantStatus: http.StatusBadRequest},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			handler, _, _ := setupTestShareHandler()
			rec := postShare(t, createShareTestRouter(handler), test.body)
			if rec.Code != test.wantStatus {
				t.Fatalf("status = %d, want %d: %s", rec.Code, test.wantStatus, rec.Body.String())
			}
		})
	}
}
