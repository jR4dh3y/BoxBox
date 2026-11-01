package handler

import (
	"context"
	"net/http"
	"net/http/httptest"
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
