package handler

import (
	"encoding/json"
	"net/http"

	"github.com/homelab/filemanager/internal/model"
)

// writeError writes an error response with the specified message, code, and status
func writeError(w http.ResponseWriter, message, code string, status int) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(model.ErrorResponse{
		Error: message,
		Code:  code,
	})
}

// writeJSON writes a JSON response with the specified data and status code
func writeJSON(w http.ResponseWriter, data interface{}, status int) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(data)
}
