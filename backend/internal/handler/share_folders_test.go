package handler

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func TestShareCreateFolderRequiresFullAccess(t *testing.T) {
	handler, fs, shareSvc := setupTestShareHandler()
	router := createShareTestRouter(handler)

	full, err := shareSvc.Create(context.Background(), "owner", "media/shared", service.ShareSettings{
		Permissions: model.SharePermissions{Upload: true, Delete: true, Manage: true},
	}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	uploadDelete, err := shareSvc.Create(context.Background(), "owner", "media/shared", service.ShareSettings{
		Permissions: model.SharePermissions{Upload: true, Delete: true},
	}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	post := func(token, folderPath string) int {
		rec := httptest.NewRecorder()
		router.ServeHTTP(rec, httptest.NewRequest(http.MethodPost, "/api/v1/share/"+token+"/folders?path="+folderPath, nil))
		return rec.Code
	}

	if code := post(full.Token, "photos/2024"); code != http.StatusCreated {
		t.Fatalf("full access create nested folder: status = %d", code)
	}
	if isDir, _ := fs.IsDir("/data/media/shared/photos/2024"); !isDir {
		t.Fatal("nested folder was not created")
	}
	if code := post(full.Token, "photos"); code != http.StatusCreated {
		t.Fatalf("creating an existing folder should succeed: status = %d", code)
	}
	if err := fs.WriteFile("/data/media/shared/file.txt", []byte("keep"), 0o644); err != nil {
		t.Fatal(err)
	}
	if code := post(full.Token, "file.txt"); code != http.StatusConflict {
		t.Fatalf("creating a folder over an existing file: status = %d", code)
	}
	if code := post(uploadDelete.Token, "other"); code != http.StatusForbidden {
		t.Fatalf("upload + delete link created a folder: status = %d", code)
	}
	if exists, _ := fs.Exists("/data/media/shared/other"); exists {
		t.Fatal("folder created without full access")
	}
	for _, escape := range []string{"../escape", "%2E%2E/escape", "/abs"} {
		if code := post(full.Token, escape); code == http.StatusCreated {
			t.Errorf("path %q escaped the share: status = %d", escape, code)
		}
	}
	if exists, _ := fs.Exists("/data/media/escape"); exists {
		t.Fatal("folder created outside the shared folder")
	}
}

func TestShareFullAccessRequiresUploadAndDelete(t *testing.T) {
	_, _, shareSvc := setupTestShareHandler()
	_, err := shareSvc.Create(context.Background(), "owner", "media/shared", service.ShareSettings{
		Permissions: model.SharePermissions{Upload: true, Manage: true},
	}, time.Time{})
	if err == nil {
		t.Fatal("full access without delete was accepted")
	}
}

func TestShareUploadCanEmptyExistingFileOnly(t *testing.T) {
	handler, fs, shareSvc := setupTestShareHandler()
	router := createShareTestRouter(handler)
	share, err := shareSvc.Create(context.Background(), "owner", "media/shared", service.ShareSettings{
		Permissions: model.SharePermissions{Upload: true, Delete: true, Manage: true},
	}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	upload := func(path, body string) int {
		rec := httptest.NewRecorder()
		router.ServeHTTP(rec, httptest.NewRequest(http.MethodPost, "/api/v1/share/"+share.Token+"/upload?path="+path, strings.NewReader(body)))
		return rec.Code
	}

	if code := upload("notes.txt", "draft"); code != http.StatusOK && code != http.StatusCreated {
		t.Fatalf("initial upload: status = %d", code)
	}
	if code := upload("notes.txt", ""); code != http.StatusOK && code != http.StatusCreated {
		t.Fatalf("emptying an existing file: status = %d", code)
	}
	if content, _ := fs.ReadFile("/data/media/shared/notes.txt"); len(content) != 0 {
		t.Fatalf("file content = %q, want empty", content)
	}
	if code := upload("new.txt", ""); code != http.StatusBadRequest {
		t.Fatalf("empty new upload: status = %d, want 400", code)
	}
}
