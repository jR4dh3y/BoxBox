//go:build linux

package main

import (
	"bufio"
	"encoding/json"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

// realSubmount returns a real (non-virtual) mount point and its parent
// directory, or skips when the host has none.
func realSubmount(t *testing.T) (parent, mountPoint string) {
	t.Helper()
	file, err := os.Open("/proc/self/mountinfo")
	if err != nil {
		t.Skipf("mountinfo unavailable: %v", err)
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		before, after, ok := strings.Cut(scanner.Text(), " - ")
		if !ok {
			continue
		}
		fields := strings.Fields(before)
		fsType := strings.Fields(after)
		if len(fields) < 5 || len(fsType) == 0 || config.VirtualFilesystems[fsType[0]] {
			continue
		}
		mp := fields[4]
		if mp == "/" || strings.Contains(mp, `\`) {
			continue
		}
		if _, err := os.ReadDir(filepath.Dir(mp)); err != nil {
			continue
		}
		return filepath.Dir(mp), mp
	}
	t.Skip("host has no real submount to discover")
	return "", ""
}

func TestInitializeServerExpandsAutoDiscoverMounts(t *testing.T) {
	parent, mountPoint := realSubmount(t)
	cfg := &model.ServerConfig{
		Host:        "127.0.0.1",
		MaxUploadMB: 10,
		ChunkSizeMB: 5,
		DataDir:     t.TempDir(),
		MountPoints: []model.MountPoint{{Name: "drives", Path: parent, AutoDiscover: true}},
	}
	server, _, _, _, _, err := initializeServer(cfg, true)
	if err != nil {
		t.Fatal(err)
	}

	rec := httptest.NewRecorder()
	server.Handler.ServeHTTP(rec, httptest.NewRequest("GET", "/api/v1/files/stats", nil))
	var body struct {
		Drives []struct{ Name, Path string }
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatalf("decode stats: %v (status %d)", err, rec.Code)
	}

	if len(body.Drives) == 0 {
		t.Fatalf("no drives returned, want submounts of %s", parent)
	}
	for _, drive := range body.Drives {
		if drive.Name == "drives" {
			t.Fatalf("auto_discover ignored: %s returned unexpanded; want submounts such as %s", parent, mountPoint)
		}
	}
}
