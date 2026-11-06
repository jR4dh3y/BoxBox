package handler

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func TestDesktopChooserMalformedSelectionRemainsPending(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	chooser := service.NewDesktopChooser(fsys, []model.MountPoint{{Name: "home", Path: "/home"}})
	request, result := chooser.Begin(service.ChooserRequest{Kind: "open"})
	defer chooser.Close()
	router := chi.NewRouter()
	RegisterDesktopChooser(router, chooser)
	for _, body := range []string{"{", `{"paths":42}`, `{"cancel":true,"unexpected":1}`, `{"cancel":true} {}`} {
		response := httptest.NewRecorder()
		router.ServeHTTP(response, httptest.NewRequest(http.MethodPost, "/chooser/"+request.ID, strings.NewReader(body)))
		if response.Code != http.StatusBadRequest {
			t.Fatalf("body %q status = %d", body, response.Code)
		}
		if _, err := chooser.Get(request.ID); err != nil {
			t.Fatalf("malformed body completed request: %v", err)
		}
	}
	if err := chooser.Select(context.Background(), request.ID, service.ChooserSelection{Cancel: true}); err != nil {
		t.Fatal(err)
	}
	if got := <-result; got.Code != 1 {
		t.Fatal(got)
	}
}

func TestDesktopChooserDirectoryAboveDefaultLimit(t *testing.T) {
	root := t.TempDir()
	for i := 0; i < 10_001; i++ {
		if err := os.WriteFile(filepath.Join(root, fmt.Sprintf("file-%05d.txt", i)), nil, 0600); err != nil {
			t.Fatal(err)
		}
	}
	chooser := service.NewDesktopChooser(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}})
	defer chooser.Close()
	request, _ := chooser.Begin(service.ChooserRequest{Kind: "open"})
	router := chi.NewRouter()
	RegisterDesktopChooser(router, chooser)
	response := httptest.NewRecorder()
	router.ServeHTTP(response, httptest.NewRequest(http.MethodGet, "/chooser/"+request.ID+"/directory?path=home", nil))
	if response.Code != http.StatusOK {
		t.Fatalf("status %d: %s", response.Code, response.Body.String())
	}
	var listing struct {
		Items []model.FileInfo `json:"items"`
	}
	if err := json.Unmarshal(response.Body.Bytes(), &listing); err != nil {
		t.Fatal(err)
	}
	if len(listing.Items) != 10_001 {
		t.Fatalf("got %d entries", len(listing.Items))
	}
}
