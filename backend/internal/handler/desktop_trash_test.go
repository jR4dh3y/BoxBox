package handler

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type stubDesktopTrashService struct {
	items     []service.DesktopTrashItem
	trashItem service.DesktopTrashItem
	err       error
	path      string
}

func (s *stubDesktopTrashService) List(context.Context) ([]service.DesktopTrashItem, error) {
	return s.items, s.err
}

func (s *stubDesktopTrashService) Trash(_ context.Context, path string) (service.DesktopTrashItem, error) {
	s.path = path
	return s.trashItem, s.err
}

func (s *stubDesktopTrashService) Restore(context.Context, string) error {
	return s.err
}

func (s *stubDesktopTrashService) Delete(context.Context, string) error {
	return s.err
}

func desktopTrashRouter(stub *stubDesktopTrashService) http.Handler {
	router := chi.NewRouter()
	NewDesktopTrashHandler(stub).RegisterRoutes(router)
	return router
}

func TestDesktopTrashHandlerRoutesListAndMove(t *testing.T) {
	item := service.DesktopTrashItem{
		ID: "entry-1", Name: "notes.txt", OriginalPath: "/home/user/notes.txt", CanRestore: true,
	}
	stub := &stubDesktopTrashService{items: []service.DesktopTrashItem{item}, trashItem: item}
	router := desktopTrashRouter(stub)

	list := httptest.NewRecorder()
	router.ServeHTTP(list, httptest.NewRequest(http.MethodGet, "/trash", nil))
	if list.Code != http.StatusOK || list.Header().Get("Cache-Control") != "no-store" {
		t.Fatalf("list response status=%d cache=%q", list.Code, list.Header().Get("Cache-Control"))
	}
	var listBody struct {
		Items []service.DesktopTrashItem `json:"items"`
	}
	if err := json.NewDecoder(list.Body).Decode(&listBody); err != nil || len(listBody.Items) != 1 || listBody.Items[0].ID != item.ID {
		t.Fatalf("list body = %#v, error = %v", listBody, err)
	}

	move := httptest.NewRecorder()
	router.ServeHTTP(move, httptest.NewRequest(http.MethodPost, "/trash", strings.NewReader(`{"path":"home/notes%20one.txt"}`)))
	if move.Code != http.StatusCreated || stub.path != "home/notes%20one.txt" {
		t.Fatalf("move status=%d path=%q", move.Code, stub.path)
	}
	var moveBody struct {
		Item service.DesktopTrashItem `json:"item"`
	}
	if err := json.NewDecoder(move.Body).Decode(&moveBody); err != nil || moveBody.Item.ID != item.ID {
		t.Fatalf("move body = %#v, error = %v", moveBody, err)
	}
}

func TestDesktopTrashHandlerValidationAndServiceErrors(t *testing.T) {
	for _, tc := range []struct {
		name       string
		body       string
		serviceErr error
		status     int
		code       string
	}{
		{name: "empty path", body: `{"path":""}`, status: http.StatusBadRequest, code: model.ErrCodeValidationError},
		{name: "unavailable", body: `{"path":"home/file"}`, serviceErr: service.ErrTrashUnavailable, status: http.StatusServiceUnavailable, code: "TRASH_UNAVAILABLE"},
		{name: "conflict", body: `{"id":"entry-1"}`, serviceErr: service.ErrTrashConflict, status: http.StatusConflict, code: model.ErrCodeConflict},
	} {
		t.Run(tc.name, func(t *testing.T) {
			stub := &stubDesktopTrashService{err: tc.serviceErr}
			recorder := httptest.NewRecorder()
			requestPath := "/trash"
			if tc.name == "conflict" {
				requestPath = "/trash/restore"
			}
			desktopTrashRouter(stub).ServeHTTP(recorder, httptest.NewRequest(http.MethodPost, requestPath, strings.NewReader(tc.body)))
			if recorder.Code != tc.status {
				t.Fatalf("status = %d, want %d", recorder.Code, tc.status)
			}
			var body model.ErrorResponse
			if err := json.NewDecoder(recorder.Body).Decode(&body); err != nil {
				t.Fatal(err)
			}
			if body.Code != tc.code {
				t.Fatalf("error code = %q, want %q", body.Code, tc.code)
			}
		})
	}
}
