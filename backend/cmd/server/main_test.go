//go:build linux

package main

import (
	"bufio"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

// realSubmount returns a mounted directory that statfs can read, and its parent,
// or skips when the host has none. File bind mounts such as /etc/hosts are not discoverable.
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
		var stat syscall.Statfs_t
		if info, err := os.Stat(mp); err != nil || !info.IsDir() || syscall.Statfs(mp, &stat) != nil {
			continue
		}
		if !isReadableDirectory(mp) {
			continue
		}
		return filepath.Dir(mp), mp
	}
	t.Skip("host has no real submount to discover")
	return "", ""
}

func isReadableDirectory(path string) bool {
	dir, err := os.Open(path)
	if err != nil {
		return false
	}
	defer dir.Close()
	_, err = dir.ReadDir(1)
	return err == nil || errors.Is(err, io.EOF)
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
	var body model.DriveStatsResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatalf("decode stats: %v (status %d)", err, rec.Code)
	}

	if len(body.Drives) == 0 {
		t.Fatalf("no drives returned, want submounts of %s", parent)
	}
	driveName := filepath.Base(mountPoint)
	found := false
	for _, drive := range body.Drives {
		if drive.Name == "drives" {
			t.Fatalf("auto_discover ignored: %s returned unexpanded; want submounts such as %s", parent, mountPoint)
		}
		found = found || drive.Name == driveName
	}
	if !found {
		t.Fatalf("drives %+v do not include the submount %s", body.Drives, mountPoint)
	}

	rootsRec := httptest.NewRecorder()
	server.Handler.ServeHTTP(rootsRec, httptest.NewRequest(http.MethodGet, "/api/v1/files/", nil))
	var roots struct {
		Roots []struct {
			Name string `json:"name"`
			Path string `json:"path"`
		}
	}
	if err := json.Unmarshal(rootsRec.Body.Bytes(), &roots); err != nil {
		t.Fatalf("decode roots: %v (status %d)", err, rootsRec.Code)
	}
	rootFound := false
	for _, root := range roots.Roots {
		if root.Name == driveName && root.Path == mountPoint {
			rootFound = true
			break
		}
	}
	if !rootFound {
		t.Fatalf("roots %+v do not include discovered mount %s at %s", roots.Roots, driveName, mountPoint)
	}

	driveRec := httptest.NewRecorder()
	drivePath := "/api/v1/files/" + url.PathEscape(driveName)
	server.Handler.ServeHTTP(driveRec, httptest.NewRequest(http.MethodGet, drivePath, nil))
	if driveRec.Code != http.StatusOK {
		t.Fatalf("open discovered drive %q: status %d, body: %s", driveName, driveRec.Code, driveRec.Body.String())
	}
}
