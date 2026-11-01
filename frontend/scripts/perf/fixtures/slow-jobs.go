package main

import (
	"encoding/json"
	"encoding/pem"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"time"
)

type root struct {
	Name     string `json:"name"`
	Path     string `json:"path"`
	ReadOnly bool   `json:"readOnly"`
}
type drive struct {
	root
	TotalBytes int `json:"totalBytes"`
	FreeBytes  int `json:"freeBytes"`
	UsedBytes  int `json:"usedBytes"`
	UsedPct    int `json:"usedPct"`
}
type file struct {
	Name        string `json:"name"`
	Path        string `json:"path"`
	Size        int    `json:"size"`
	IsDir       bool   `json:"isDir"`
	ModTime     string `json:"modTime"`
	Permissions string `json:"permissions"`
}
type job struct {
	ID         string `json:"id"`
	Type       string `json:"type"`
	State      string `json:"state"`
	Progress   int    `json:"progress"`
	SourcePath string `json:"sourcePath"`
	CreatedAt  string `json:"createdAt"`
}

func main() {
	dir := os.Args[1]
	s := httptest.NewTLSServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		if r.URL.Path == "/api/v1/connect/redeem" {
			json.NewEncoder(w).Encode(struct {
				ClientID   string    `json:"clientId"`
				Credential string    `json:"credential"`
				Username   string    `json:"username"`
				ExpiresAt  time.Time `json:"expiresAt"`
			}{"fixture-client", "boxbox_client_" + strings.Repeat("A", 43), "fixture", time.Now().Add(time.Hour)})
			return
		}
		if r.Header.Get("Authorization") != "Bearer boxbox_client_"+strings.Repeat("A", 43) {
			http.Error(w, "auth required", 401)
			return
		}
		b, _ := os.ReadFile(filepath.Join(dir, "state.json"))
		var state struct {
			Names    []string `json:"names"`
			Root     string   `json:"root"`
			Delay    int      `json:"delay"`
			JobState string   `json:"jobState"`
			Progress int      `json:"progress"`
		}
		json.Unmarshal(b, &state)
		switch r.URL.Path {
		case "/api/v1/jobs":
			time.Sleep(time.Duration(state.Delay) * time.Millisecond)
			json.NewEncoder(w).Encode(map[string][]job{"jobs": {{ID: "remote-job", Type: "copy", State: state.JobState, Progress: state.Progress, SourcePath: "shared/seed.txt", CreatedAt: "2026-10-25T00:00:00Z"}}})
		case "/api/v1/files":
			json.NewEncoder(w).Encode(map[string][]root{"roots": {{Name: state.Root, Path: state.Root}}})
		case "/api/v1/files/stats":
			json.NewEncoder(w).Encode(map[string][]drive{"drives": {{root: root{Name: state.Root, Path: state.Root}, TotalBytes: 100, FreeBytes: 50, UsedBytes: 50, UsedPct: 50}}})
		case "/api/v1/files/list/shared":
			items := []file{}
			for _, name := range state.Names {
				items = append(items, file{Name: name, Path: "shared/" + name, Size: 1})
			}
			json.NewEncoder(w).Encode(struct {
				Path  string `json:"path"`
				Items []file `json:"items"`
			}{"shared", items})
		default:
			http.Error(w, "unavailable fixture route", 404)
		}
	}))
	defer s.Close()
	os.WriteFile(filepath.Join(dir, "ca.pem"), pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: s.Certificate().Raw}), 0600)
	fmt.Println(s.URL)
	select {}
}
