package handler

import (
	"bufio"
	"context"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func TestDesktopWatchExternalChanges(t *testing.T) {
	dir := t.TempDir()
	handler := DesktopWatch(service.NewDesktopWatcher(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: dir}}))
	server := httptest.NewServer(handler)
	defer server.Close()
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	request, _ := http.NewRequestWithContext(ctx, "GET", server.URL+"?path=home", nil)
	response, err := http.DefaultClient.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	reader := bufio.NewScanner(response.Body)
	if !reader.Scan() || reader.Text() != "event: ready" {
		t.Fatal("watch did not become ready")
	}
	if err := os.WriteFile(filepath.Join(dir, "from-another-app.txt"), []byte("saved"), 0600); err != nil {
		t.Fatal(err)
	}
	for reader.Scan() {
		if reader.Text() == "event: change" {
			return
		}
	}
	t.Fatal("external write did not reach the browser stream")
}

func TestDesktopWatchConfinement(t *testing.T) {
	dir := t.TempDir()
	if err := os.Symlink(t.TempDir(), filepath.Join(dir, "escape")); err != nil {
		t.Fatal(err)
	}
	handler := DesktopWatch(service.NewDesktopWatcher(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: dir}}))
	for _, path := range []string{"home/escape", "other", "home/../outside"} {
		response := httptest.NewRecorder()
		handler.ServeHTTP(response, httptest.NewRequest("GET", "/?path="+path, nil))
		if response.Code == 200 {
			t.Fatalf("watch accepted %q", path)
		}
	}
}
