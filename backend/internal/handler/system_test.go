package handler

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func TestGetUploadConfigReturnsConfiguredChunkSize(t *testing.T) {
	tests := []struct {
		name        string
		chunkSizeMB int
		want        int64
	}{
		{"configured", 5, 5 << 20},
		{"unset falls back to default", 0, int64(config.DefaultChunkSizeMB) << 20},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			handler := NewSystemHandler(service.NewSystemService(), tt.chunkSizeMB)
			rec := httptest.NewRecorder()
			handler.GetUploadConfig(rec, httptest.NewRequest(http.MethodGet, "/api/v1/system/upload", nil))

			var body UploadConfigResponse
			if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
				t.Fatal(err)
			}
			if rec.Code != http.StatusOK || body.ChunkSizeBytes != tt.want {
				t.Fatalf("status %d chunkSizeBytes %d, want 200 and %d", rec.Code, body.ChunkSizeBytes, tt.want)
			}
		})
	}
}
