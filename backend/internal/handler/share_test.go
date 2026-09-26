package handler

import (
	"archive/zip"
	"bytes"
	"context"
	"encoding/json"
	"io"
	"mime"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/authcontext"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

func setupTestShareHandler() (*ShareHandler, *filesystem.AferoFS, service.ShareService) {
	fs := filesystem.NewMemMapFS()
	_ = fs.MkdirAll("/data", 0755)
	_ = fs.MkdirAll("/data/media", 0755)
	_ = fs.MkdirAll("/data/media/shared", 0755)
	_ = fs.MkdirAll("/data/archive", 0755)
	_ = fs.WriteFile("/data/media/file.txt", []byte("shared content"), 0644)
	_ = fs.WriteFile("/data/archive/file.txt", []byte("archived content"), 0644)

	mounts := []model.MountPoint{
		{Name: "media", Path: "/data/media", ReadOnly: false},
		{Name: "archive", Path: "/data/archive", ReadOnly: true},
	}
	shareSvc := service.NewShareService(fs, service.ShareServiceConfig{
		DataDir: "/data",
		Mounts:  func() []model.MountPoint { return mounts },
	})
	return NewShareHandler(shareSvc, 1), fs, shareSvc
}

func createShareTestRouter(handler *ShareHandler) *chi.Mux {
	r := chi.NewRouter()
	r.Route("/api/v1", func(r chi.Router) {
		r.Route("/shares", func(r chi.Router) {
			handler.RegisterRoutes(r)
		})
		r.Route("/share", func(r chi.Router) {
			handler.RegisterPublicRoutes(r)
		})
	})
	return r
}

// newShareManagementRequest builds an authenticated management request the way
// the JWT middleware would: with the username in the request context.
func newShareManagementRequest(method, target string, body io.Reader) *http.Request {
	return newShareManagementRequestForUser("owner", method, target, body)
}

func newShareManagementRequestForUser(username, method, target string, body io.Reader) *http.Request {
	req := httptest.NewRequest(method, target, body)
	return req.WithContext(authcontext.WithUsername(req.Context(), username))
}

func createShareViaAPI(t *testing.T, router *chi.Mux, path string, permissions model.SharePermissions, expiresInSeconds *int64) model.ShareResponse {
	return createShareViaAPIWithOptions(t, router, path, permissions, expiresInSeconds, nil)
}

func createShareViaAPIWithLimit(t *testing.T, router *chi.Mux, path string, permissions model.SharePermissions, maxUploadBytes int64) model.ShareResponse {
	return createShareViaAPIWithOptions(t, router, path, permissions, nil, &maxUploadBytes)
}

func createShareViaAPIWithOptions(t *testing.T, router *chi.Mux, path string, permissions model.SharePermissions, expiresInSeconds *int64, maxUploadBytes *int64) model.ShareResponse {
	t.Helper()

	body := map[string]any{"path": path, "permissions": permissions}
	if expiresInSeconds != nil {
		body["expiresInSeconds"] = *expiresInSeconds
	}
	if maxUploadBytes != nil {
		body["maxUploadBytes"] = *maxUploadBytes
	}
	encoded, err := json.Marshal(body)
	if err != nil {
		t.Fatal(err)
	}
	req := newShareManagementRequest(http.MethodPost, "/api/v1/shares", bytes.NewReader(encoded))
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)
	if rec.Code != http.StatusCreated {
		t.Fatalf("create share status = %d body=%s", rec.Code, rec.Body.String())
	}

	var response model.ShareResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &response); err != nil {
		t.Fatal(err)
	}
	return response
}

func TestCreateShareViaAPI(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)

	expiresInSeconds := int64(3600)
	response := createShareViaAPI(
		t,
		router,
		"media/file.txt",
		model.SharePermissions{View: true, Download: true},
		&expiresInSeconds,
	)

	if response.ID == "" || len(response.Token) != 43 {
		t.Fatalf("unexpected create response: %+v", response)
	}
	if response.URL != "/s/"+response.Token {
		t.Fatalf("url = %q, want /s/%s", response.URL, response.Token)
	}
	if response.FileName != "file.txt" {
		t.Fatalf("fileName = %q, want file.txt", response.FileName)
	}
	if !response.Permissions.View || !response.Permissions.Download || response.Permissions.Upload || response.Permissions.Delete {
		t.Fatalf("permissions = %+v", response.Permissions)
	}
	if response.ExpiresAt.IsZero() {
		t.Fatal("expiresAt missing for a time-limited share")
	}
}

func TestCreateShareRejectsInvalidTargetsViaAPI(t *testing.T) {
	tests := []struct {
		name       string
		path       string
		perms      model.SharePermissions
		wantStatus int
	}{
		{name: "missing file", path: "media/missing.txt", perms: model.SharePermissions{View: true}, wantStatus: http.StatusNotFound},
		{name: "empty path", path: "", perms: model.SharePermissions{View: true}, wantStatus: http.StatusBadRequest},
		{name: "negative expiry", path: "media/file.txt", perms: model.SharePermissions{View: true}, wantStatus: http.StatusBadRequest},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			handler, _, _ := setupTestShareHandler()
			router := createShareTestRouter(handler)

			expiresInSeconds := int64(-5)
			body := map[string]any{"path": test.path, "permissions": test.perms}
			if test.name == "negative expiry" {
				body["expiresInSeconds"] = expiresInSeconds
			}
			encoded, err := json.Marshal(body)
			if err != nil {
				t.Fatal(err)
			}

			req := newShareManagementRequest(http.MethodPost, "/api/v1/shares", bytes.NewReader(encoded))
			rec := httptest.NewRecorder()
			router.ServeHTTP(rec, req)

			if rec.Code != test.wantStatus {
				t.Fatalf("status = %d, want %d: %s", rec.Code, test.wantStatus, rec.Body.String())
			}
		})
	}
}

func TestCreateFolderShareViaAPI(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)

	response := createShareViaAPI(t, router, "media/shared", model.SharePermissions{Upload: true}, nil)
	if !response.IsFolder || !response.Permissions.View || !response.Permissions.Download || !response.Permissions.Upload || response.Permissions.Delete {
		t.Fatalf("folder response = %+v", response)
	}
}

func TestShareManagementRequiresUsername(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)

	tests := []struct {
		name   string
		method string
		target string
		body   string
	}{
		{name: "create", method: http.MethodPost, target: "/api/v1/shares", body: `{"path":"media/file.txt","permissions":{"view":true}}`},
		{name: "list", method: http.MethodGet, target: "/api/v1/shares"},
		{name: "revoke", method: http.MethodDelete, target: "/api/v1/shares/share-id"},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			var body io.Reader
			if test.body != "" {
				body = strings.NewReader(test.body)
			}
			req := httptest.NewRequest(test.method, test.target, body)
			rec := httptest.NewRecorder()
			router.ServeHTTP(rec, req)

			if rec.Code != http.StatusUnauthorized {
				t.Fatalf("status = %d, want %d", rec.Code, http.StatusUnauthorized)
			}
		})
	}
}

func TestShareLegacyReplacementCapabilityInResponses(t *testing.T) {
	handler, _, shareService := setupTestShareHandler()
	router := createShareTestRouter(handler)
	share, err := shareService.Create(context.Background(), "owner", "media/shared", service.ShareSettings{
		Permissions: model.SharePermissions{Upload: true, LegacyReplace: true},
	}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	infoReq := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token, nil)
	infoRec := httptest.NewRecorder()
	router.ServeHTTP(infoRec, infoReq)
	if infoRec.Code != http.StatusOK {
		t.Fatalf("share info status = %d, want %d: %s", infoRec.Code, http.StatusOK, infoRec.Body.String())
	}
	var info model.ShareInfoResponse
	if err := json.NewDecoder(infoRec.Body).Decode(&info); err != nil {
		t.Fatal(err)
	}
	if !info.Permissions.CanReplace || info.Permissions.Delete {
		t.Fatalf("recipient permissions = %+v, want replace without delete", info.Permissions)
	}

	listReq := newShareManagementRequest(http.MethodGet, "/api/v1/shares", nil)
	listRec := httptest.NewRecorder()
	router.ServeHTTP(listRec, listReq)
	if listRec.Code != http.StatusOK {
		t.Fatalf("share list status = %d, want %d: %s", listRec.Code, http.StatusOK, listRec.Body.String())
	}
	var list model.ShareListResponse
	if err := json.NewDecoder(listRec.Body).Decode(&list); err != nil {
		t.Fatal(err)
	}
	if len(list.Shares) != 1 || !list.Shares[0].Permissions.CanReplace || list.Shares[0].Permissions.Delete {
		t.Fatalf("owner share permissions = %+v, want replace without delete", list.Shares)
	}
}

func TestShareListAndRevokeViaAPI(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)

	first := createShareViaAPI(t, router, "media/file.txt", model.SharePermissions{View: true}, nil)
	second := createShareViaAPI(t, router, "media/file.txt", model.SharePermissions{Download: true}, nil)

	listReq := newShareManagementRequest(http.MethodGet, "/api/v1/shares", nil)
	listRec := httptest.NewRecorder()
	router.ServeHTTP(listRec, listReq)
	if listRec.Code != http.StatusOK {
		t.Fatalf("list status = %d: %s", listRec.Code, listRec.Body.String())
	}

	var list model.ShareListResponse
	if err := json.Unmarshal(listRec.Body.Bytes(), &list); err != nil {
		t.Fatal(err)
	}
	if len(list.Shares) != 2 {
		t.Fatalf("list returned %d shares, want 2", len(list.Shares))
	}
	summary := list.Shares[0]
	if summary.Token == "" || summary.URL != "/s/"+summary.Token {
		t.Fatalf("summary token/url = %+v", summary)
	}
	if summary.Path != "media/file.txt" {
		t.Fatalf("summary path = %q, want media/file.txt", summary.Path)
	}
	if summary.FileName != "file.txt" {
		t.Fatalf("summary fileName = %q", summary.FileName)
	}

	revokeReq := newShareManagementRequest(http.MethodDelete, "/api/v1/shares/"+first.ID, nil)
	revokeRec := httptest.NewRecorder()
	router.ServeHTTP(revokeRec, revokeReq)
	if revokeRec.Code != http.StatusOK {
		t.Fatalf("revoke status = %d: %s", revokeRec.Code, revokeRec.Body.String())
	}

	listReq = newShareManagementRequest(http.MethodGet, "/api/v1/shares", nil)
	listRec = httptest.NewRecorder()
	router.ServeHTTP(listRec, listReq)
	var updated model.ShareListResponse
	if err := json.Unmarshal(listRec.Body.Bytes(), &updated); err != nil {
		t.Fatal(err)
	}
	if len(updated.Shares) != 1 || updated.Shares[0].ID != second.ID {
		t.Fatalf("list after revoke = %+v, want only %q", updated.Shares, second.ID)
	}

	unknownReq := newShareManagementRequest(http.MethodDelete, "/api/v1/shares/missing-id", nil)
	unknownRec := httptest.NewRecorder()
	router.ServeHTTP(unknownRec, unknownReq)
	if unknownRec.Code != http.StatusNotFound {
		t.Fatalf("revoke unknown status = %d, want 404", unknownRec.Code)
	}
}

func TestSharePermissionOnlyUpdatePreservesUploadLimit(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)
	share := createShareViaAPIWithLimit(t, router, "media/shared", model.SharePermissions{Upload: true}, 8)

	updateReq := newShareManagementRequest(http.MethodPatch, "/api/v1/shares/"+share.ID, strings.NewReader(`{"permissions":{"upload":true}}`))
	updateRec := httptest.NewRecorder()
	router.ServeHTTP(updateRec, updateReq)
	if updateRec.Code != http.StatusOK {
		t.Fatalf("permissions-only update status = %d, want 200: %s", updateRec.Code, updateRec.Body.String())
	}
	var updated model.ShareSummary
	if err := json.Unmarshal(updateRec.Body.Bytes(), &updated); err != nil {
		t.Fatal(err)
	}
	if updated.MaxUploadBytes != 8 {
		t.Fatalf("permissions-only update raised upload limit to %d, want 8", updated.MaxUploadBytes)
	}

	uploadReq := httptest.NewRequest(http.MethodPost, "/api/v1/share/"+share.Token+"/upload?path=too-large.txt", strings.NewReader("123456789"))
	uploadRec := httptest.NewRecorder()
	router.ServeHTTP(uploadRec, uploadReq)
	if uploadRec.Code != http.StatusRequestEntityTooLarge {
		t.Fatalf("upload over preserved limit status = %d, want 413: %s", uploadRec.Code, uploadRec.Body.String())
	}
}

func TestShareManagementIsScopedToOwnerViaAPI(t *testing.T) {
	handler, _, shareSvc := setupTestShareHandler()
	router := createShareTestRouter(handler)

	owner := createShareViaAPI(t, router, "media/file.txt", model.SharePermissions{View: true}, nil)
	other, err := shareSvc.Create(context.Background(), "other", "media/file.txt", service.ShareSettings{Permissions: model.SharePermissions{Download: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}

	listReq := newShareManagementRequestForUser("other", http.MethodGet, "/api/v1/shares", nil)
	listRec := httptest.NewRecorder()
	router.ServeHTTP(listRec, listReq)
	if listRec.Code != http.StatusOK {
		t.Fatalf("list status = %d: %s", listRec.Code, listRec.Body.String())
	}
	var list model.ShareListResponse
	if err := json.Unmarshal(listRec.Body.Bytes(), &list); err != nil {
		t.Fatal(err)
	}
	if len(list.Shares) != 1 || list.Shares[0].ID != other.ID {
		t.Fatalf("other user's list = %+v, want only %q", list.Shares, other.ID)
	}
	if strings.Contains(listRec.Body.String(), owner.Token) {
		t.Fatalf("other user's list leaked owner token: %s", listRec.Body.String())
	}

	revokeReq := newShareManagementRequestForUser("other", http.MethodDelete, "/api/v1/shares/"+owner.ID, nil)
	revokeRec := httptest.NewRecorder()
	router.ServeHTTP(revokeRec, revokeReq)
	if revokeRec.Code != http.StatusNotFound {
		t.Fatalf("cross-owner revoke status = %d, want 404", revokeRec.Code)
	}
	if _, err := shareSvc.ResolveForRecipient(owner.Token); err != nil {
		t.Fatalf("owner share was revoked by another user: %v", err)
	}
}

func TestShareInfoOmitsInternalPaths(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)

	share := createShareViaAPI(t, router, "media/file.txt", model.SharePermissions{View: true, Download: true}, nil)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token, nil)
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("info status = %d: %s", rec.Code, rec.Body.String())
	}
	body := rec.Body.String()
	for _, leaked := range []string{"media", "/data", "mountName", "relPath"} {
		if strings.Contains(body, leaked) {
			t.Fatalf("info response leaked %q: %s", leaked, body)
		}
	}

	var info model.ShareInfoResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &info); err != nil {
		t.Fatal(err)
	}
	if info.FileName != "file.txt" || info.Size != int64(len("shared content")) || info.MimeType == "" {
		t.Fatalf("info = %+v", info)
	}
	if !info.Permissions.View || !info.Permissions.Download || info.Permissions.Upload || info.Permissions.Delete {
		t.Fatalf("info permissions = %+v", info.Permissions)
	}
}

func TestShareFolderItemsAndNestedDownloadViaAPI(t *testing.T) {
	handler, fs, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)
	if err := fs.WriteFile("/data/media/shared/notes.txt", []byte("notes"), 0o644); err != nil {
		t.Fatal(err)
	}
	share := createShareViaAPI(t, router, "media/shared", model.SharePermissions{}, nil)

	itemsReq := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token+"/items", nil)
	itemsRec := httptest.NewRecorder()
	router.ServeHTTP(itemsRec, itemsReq)
	if itemsRec.Code != http.StatusOK {
		t.Fatalf("items status = %d: %s", itemsRec.Code, itemsRec.Body.String())
	}
	var items model.ShareDirectoryResponse
	if err := json.Unmarshal(itemsRec.Body.Bytes(), &items); err != nil {
		t.Fatal(err)
	}
	if items.Path != "" || len(items.Items) != 1 || items.Items[0].Path != "notes.txt" {
		t.Fatalf("folder items = %+v", items)
	}

	downloadReq := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token+"/download?path=notes.txt", nil)
	downloadRec := httptest.NewRecorder()
	router.ServeHTTP(downloadRec, downloadReq)
	if downloadRec.Code != http.StatusOK || downloadRec.Body.String() != "notes" {
		t.Fatalf("nested download status=%d body=%q", downloadRec.Code, downloadRec.Body.String())
	}
}

func TestShareFolderRejectsInvalidPathsViaAPI(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)
	share := createShareViaAPI(t, router, "media/shared", model.SharePermissions{Upload: true}, nil)

	traversalReq := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token+"/items?path=../media", nil)
	traversalRec := httptest.NewRecorder()
	router.ServeHTTP(traversalRec, traversalReq)
	if traversalRec.Code != http.StatusBadRequest {
		t.Fatalf("traversal listing status = %d, want 400: %s", traversalRec.Code, traversalRec.Body.String())
	}

	missingReq := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token+"/download?path=missing.txt", nil)
	missingRec := httptest.NewRecorder()
	router.ServeHTTP(missingRec, missingReq)
	if missingRec.Code != http.StatusNotFound {
		t.Fatalf("missing nested download status = %d, want 404: %s", missingRec.Code, missingRec.Body.String())
	}

	uploadReq := httptest.NewRequest(http.MethodPost, "/api/v1/share/"+share.Token+"/upload?path=../escape.txt", strings.NewReader("escape"))
	uploadRec := httptest.NewRecorder()
	router.ServeHTTP(uploadRec, uploadReq)
	if uploadRec.Code != http.StatusBadRequest {
		t.Fatalf("traversal upload status = %d, want 400: %s", uploadRec.Code, uploadRec.Body.String())
	}

	missingParentReq := httptest.NewRequest(http.MethodPost, "/api/v1/share/"+share.Token+"/upload?path=missing/child.txt", strings.NewReader("child"))
	missingParentRec := httptest.NewRecorder()
	router.ServeHTTP(missingParentRec, missingParentReq)
	if missingParentRec.Code != http.StatusNotFound {
		t.Fatalf("missing parent upload status = %d, want 404: %s", missingParentRec.Code, missingParentRec.Body.String())
	}
}

func TestShareFolderViewerAndReadOnlyMountCannotUploadViaAPI(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)

	viewer := createShareViaAPI(t, router, "media/shared", model.SharePermissions{}, nil)
	viewerReq := httptest.NewRequest(http.MethodPost, "/api/v1/share/"+viewer.Token+"/upload?path=notes.txt", strings.NewReader("notes"))
	viewerRec := httptest.NewRecorder()
	router.ServeHTTP(viewerRec, viewerReq)
	if viewerRec.Code != http.StatusForbidden {
		t.Fatalf("viewer upload status = %d, want 403", viewerRec.Code)
	}

	readOnly := createShareViaAPI(t, router, "archive", model.SharePermissions{}, nil)
	readOnlyReq := httptest.NewRequest(http.MethodPost, "/api/v1/share/"+readOnly.Token+"/upload?path=file.txt", strings.NewReader("blocked"))
	readOnlyRec := httptest.NewRecorder()
	router.ServeHTTP(readOnlyRec, readOnlyReq)
	if readOnlyRec.Code != http.StatusForbidden {
		t.Fatalf("read-only folder upload status = %d, want 403", readOnlyRec.Code)
	}
}

func TestShareFileUploadIsRejectedViaAPI(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)
	share := createShareViaAPI(t, router, "media/file.txt", model.SharePermissions{Upload: true}, nil)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/share/"+share.Token+"/upload", strings.NewReader("blocked"))
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)
	if rec.Code != http.StatusForbidden {
		t.Fatalf("file upload status = %d, want 403", rec.Code)
	}
}

func TestShareRecipientEndpointsAreUniformlyNotFound(t *testing.T) {
	handler, _, shareSvc := setupTestShareHandler()
	router := createShareTestRouter(handler)
	perms := service.ShareSettings{Permissions: model.SharePermissions{View: true, Download: true, Upload: true}}

	expired, err := shareSvc.Create(context.Background(), "owner", "media/file.txt", perms, time.Now().Add(-time.Hour))
	if err != nil {
		t.Fatal(err)
	}
	revoked, err := shareSvc.Create(context.Background(), "owner", "media/file.txt", perms, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if err := shareSvc.Revoke("owner", revoked.ID); err != nil {
		t.Fatal(err)
	}

	tests := []struct {
		name  string
		token string
	}{
		{name: "unknown token", token: "unknown-token"},
		{name: "expired share", token: expired.Token},
		{name: "revoked share", token: revoked.Token},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			for _, route := range []string{"", "/download", "/preview", "/upload"} {
				method := http.MethodGet
				var body io.Reader
				if route == "/upload" {
					method = http.MethodPost
					body = strings.NewReader("data")
				}
				req := httptest.NewRequest(method, "/api/v1/share/"+test.token+route, body)
				rec := httptest.NewRecorder()
				router.ServeHTTP(rec, req)

				if rec.Code != http.StatusNotFound {
					t.Fatalf("%s status = %d, want 404: %s", route, rec.Code, rec.Body.String())
				}
				var errResponse model.ErrorResponse
				if err := json.Unmarshal(rec.Body.Bytes(), &errResponse); err != nil {
					t.Fatal(err)
				}
				if errResponse.Code != model.ErrCodeNotFound {
					t.Fatalf("%s code = %q, want %q", route, errResponse.Code, model.ErrCodeNotFound)
				}
			}
		})
	}
}

func TestShareDownloadServesAttachmentWithSandboxCSP(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)

	share := createShareViaAPI(t, router, "media/file.txt", model.SharePermissions{Download: true}, nil)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token+"/download", nil)
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("download status = %d: %s", rec.Code, rec.Body.String())
	}
	if got := rec.Header().Get("Content-Security-Policy"); got != streamSandboxCSP {
		t.Fatalf("Content-Security-Policy = %q, want %q", got, streamSandboxCSP)
	}
	disposition, params, err := mime.ParseMediaType(rec.Header().Get("Content-Disposition"))
	if err != nil {
		t.Fatal(err)
	}
	if disposition != "attachment" || params["filename"] != "file.txt" {
		t.Fatalf("disposition = %q filename = %q", disposition, params["filename"])
	}
	if rec.Body.String() != "shared content" {
		t.Fatalf("download body = %q", rec.Body.String())
	}

	rangeReq := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token+"/download", nil)
	rangeReq.Header.Set("Range", "bytes=0-3")
	rangeRec := httptest.NewRecorder()
	router.ServeHTTP(rangeRec, rangeReq)
	if rangeRec.Code != http.StatusPartialContent {
		t.Fatalf("range status = %d, want 206", rangeRec.Code)
	}
	if rangeRec.Body.String() != "shar" {
		t.Fatalf("range body = %q, want shar", rangeRec.Body.String())
	}
}

func TestSharePreviewSandboxesActiveDocuments(t *testing.T) {
	tests := []struct {
		name            string
		filename        string
		content         []byte
		wantDisposition string
	}{
		{
			name:            "HTML is downloaded",
			filename:        "attack.html",
			content:         []byte(`<script>window.top.location = "https://attacker.example"</script>`),
			wantDisposition: "attachment",
		},
		{
			name:            "text stays inline",
			filename:        "notes.txt",
			content:         []byte("plain text"),
			wantDisposition: "inline",
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			handler, fs, _ := setupTestShareHandler()
			router := createShareTestRouter(handler)
			if err := fs.WriteFile("/data/media/"+test.filename, test.content, 0o644); err != nil {
				t.Fatal(err)
			}

			share := createShareViaAPI(t, router, "media/"+test.filename, model.SharePermissions{View: true}, nil)

			req := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token+"/preview", nil)
			rec := httptest.NewRecorder()
			router.ServeHTTP(rec, req)

			if rec.Code != http.StatusOK {
				t.Fatalf("preview status = %d: %s", rec.Code, rec.Body.String())
			}
			if got := rec.Header().Get("Content-Security-Policy"); got != streamSandboxCSP {
				t.Fatalf("Content-Security-Policy = %q, want %q", got, streamSandboxCSP)
			}
			disposition, _, err := mime.ParseMediaType(rec.Header().Get("Content-Disposition"))
			if err != nil {
				t.Fatal(err)
			}
			if disposition != test.wantDisposition {
				t.Fatalf("disposition = %q, want %q", disposition, test.wantDisposition)
			}
		})
	}
}

func TestShareStreamingRequiresPermission(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)

	viewOnly := createShareViaAPI(t, router, "media/file.txt", model.SharePermissions{View: true}, nil)

	downloadReq := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+viewOnly.Token+"/download", nil)
	downloadRec := httptest.NewRecorder()
	router.ServeHTTP(downloadRec, downloadReq)
	if downloadRec.Code != http.StatusOK {
		t.Fatalf("file download status = %d, want 200", downloadRec.Code)
	}

	downloadOnly := createShareViaAPI(t, router, "media/file.txt", model.SharePermissions{Download: true}, nil)
	previewReq := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+downloadOnly.Token+"/preview", nil)
	previewRec := httptest.NewRecorder()
	router.ServeHTTP(previewRec, previewReq)
	if previewRec.Code != http.StatusOK {
		t.Fatalf("file preview status = %d, want 200", previewRec.Code)
	}
}

func TestShareFolderUploadOverwritesFileViaAPI(t *testing.T) {
	handler, fs, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)

	share := createShareViaAPI(t, router, "media", model.SharePermissions{Upload: true, Delete: true}, nil)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/share/"+share.Token+"/upload?path=file.txt", strings.NewReader("new contents"))
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("upload status = %d: %s", rec.Code, rec.Body.String())
	}
	var response model.ShareUploadResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &response); err != nil {
		t.Fatal(err)
	}
	if response.FileName != "file.txt" || response.Size != int64(len("new contents")) {
		t.Fatalf("upload response = %+v", response)
	}

	content, err := fs.ReadFile("/data/media/file.txt")
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != "new contents" {
		t.Fatalf("uploaded content = %q, want %q", content, "new contents")
	}
	entries, err := fs.ReadDir("/data/media")
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) != 2 || entries[0].Name() != "file.txt" || entries[1].Name() != "shared" {
		t.Fatalf("unexpected leftover entries after upload: %v", entries)
	}
}

func TestShareUploadRejectsInvalidRequests(t *testing.T) {
	handler, _, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)

	readOnly := createShareViaAPI(t, router, "media", model.SharePermissions{}, nil)
	writable := createShareViaAPI(t, router, "media", model.SharePermissions{Upload: true}, nil)

	tests := []struct {
		name       string
		token      string
		body       io.Reader
		wantStatus int
	}{
		{name: "share without write permission", token: readOnly.Token, body: strings.NewReader("data"), wantStatus: http.StatusForbidden},
		{name: "unknown token", token: "unknown-token", body: strings.NewReader("data"), wantStatus: http.StatusNotFound},
		{name: "empty body", token: writable.Token, body: nil, wantStatus: http.StatusBadRequest},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodPost, "/api/v1/share/"+test.token+"/upload", test.body)
			rec := httptest.NewRecorder()
			router.ServeHTTP(rec, req)

			if rec.Code != test.wantStatus {
				t.Fatalf("status = %d, want %d: %s", rec.Code, test.wantStatus, rec.Body.String())
			}
		})
	}
}

func TestShareUploadEnforcesMaxBytes(t *testing.T) {
	// NewShareHandler(_, 1) caps uploads at 1 MiB.
	handler, fs, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)
	share := createShareViaAPI(t, router, "media", model.SharePermissions{Upload: true}, nil)

	oversized := bytes.Repeat([]byte("a"), 1<<20+1)

	// A declared Content-Length above the limit is rejected up front.
	declaredReq := httptest.NewRequest(http.MethodPost, "/api/v1/share/"+share.Token+"/upload?path=large.bin", bytes.NewReader(oversized))
	declaredRec := httptest.NewRecorder()
	router.ServeHTTP(declaredRec, declaredReq)
	if declaredRec.Code != http.StatusRequestEntityTooLarge {
		t.Fatalf("declared oversize status = %d, want 413", declaredRec.Code)
	}

	// A chunked body that lies about its size is cut off by MaxBytesReader.
	chunkedReq := httptest.NewRequest(
		http.MethodPost,
		"/api/v1/share/"+share.Token+"/upload?path=large.bin",
		io.NopCloser(bytes.NewReader(oversized)),
	)
	chunkedReq.ContentLength = -1
	chunkedRec := httptest.NewRecorder()
	router.ServeHTTP(chunkedRec, chunkedReq)
	if chunkedRec.Code != http.StatusRequestEntityTooLarge {
		t.Fatalf("chunked oversize status = %d, want 413: %s", chunkedRec.Code, chunkedRec.Body.String())
	}

	content, err := fs.ReadFile("/data/media/file.txt")
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != "shared content" {
		t.Fatalf("target content after rejected uploads = %q, want original", content)
	}
}

func TestShareUploadOnlyRespectsPerLinkLimitAndCannotOverwriteViaAPI(t *testing.T) {
	handler, fs, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)
	share := createShareViaAPIWithLimit(t, router, "media", model.SharePermissions{Upload: true}, 4)
	if share.MaxUploadBytes != 4 {
		t.Fatalf("share upload limit = %d, want 4", share.MaxUploadBytes)
	}

	denied := httptest.NewRequest(http.MethodPost, "/api/v1/share/"+share.Token+"/upload?path=file.txt", strings.NewReader("no"))
	deniedRec := httptest.NewRecorder()
	router.ServeHTTP(deniedRec, denied)
	if deniedRec.Code != http.StatusForbidden {
		t.Fatalf("upload-only overwrite status = %d, want 403: %s", deniedRec.Code, deniedRec.Body.String())
	}

	allowed := httptest.NewRequest(http.MethodPost, "/api/v1/share/"+share.Token+"/upload?path=new.txt", strings.NewReader("four"))
	allowedRec := httptest.NewRecorder()
	router.ServeHTTP(allowedRec, allowed)
	if allowedRec.Code != http.StatusOK {
		t.Fatalf("upload-only create status = %d, want 200: %s", allowedRec.Code, allowedRec.Body.String())
	}

	tooLarge := httptest.NewRequest(http.MethodPost, "/api/v1/share/"+share.Token+"/upload?path=large.txt", strings.NewReader("five!"))
	tooLargeRec := httptest.NewRecorder()
	router.ServeHTTP(tooLargeRec, tooLarge)
	if tooLargeRec.Code != http.StatusRequestEntityTooLarge {
		t.Fatalf("per-link oversize status = %d, want 413: %s", tooLargeRec.Code, tooLargeRec.Body.String())
	}
	if exists, err := fs.Exists("/data/media/large.txt"); err != nil || exists {
		t.Fatalf("oversize upload left a file (exists=%t, err=%v)", exists, err)
	}
}

func TestShareUpdateAndRecipientDeleteViaAPI(t *testing.T) {
	handler, fs, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)
	if err := fs.WriteFile("/data/media/shared/note.txt", []byte("note"), 0o644); err != nil {
		t.Fatal(err)
	}
	share := createShareViaAPIWithLimit(t, router, "media/shared", model.SharePermissions{Upload: true}, 8)

	deletePath := "/api/v1/share/" + share.Token + "/items?path=note.txt"
	deniedDelete := httptest.NewRequest(http.MethodDelete, deletePath, nil)
	deniedDeleteRec := httptest.NewRecorder()
	router.ServeHTTP(deniedDeleteRec, deniedDelete)
	if deniedDeleteRec.Code != http.StatusForbidden {
		t.Fatalf("upload-only delete status = %d, want 403: %s", deniedDeleteRec.Code, deniedDeleteRec.Body.String())
	}

	maxUploadBytes := int64(16)
	updatedRequest := model.UpdateShareRequest{
		Permissions:    model.SharePermissions{Upload: true, Delete: true},
		MaxUploadBytes: &maxUploadBytes,
	}
	updatedBody, err := json.Marshal(updatedRequest)
	if err != nil {
		t.Fatal(err)
	}
	updateReq := newShareManagementRequest(http.MethodPatch, "/api/v1/shares/"+share.ID, bytes.NewReader(updatedBody))
	updateRec := httptest.NewRecorder()
	router.ServeHTTP(updateRec, updateReq)
	if updateRec.Code != http.StatusOK {
		t.Fatalf("update status = %d, want 200: %s", updateRec.Code, updateRec.Body.String())
	}
	var updated model.ShareSummary
	if err := json.Unmarshal(updateRec.Body.Bytes(), &updated); err != nil {
		t.Fatal(err)
	}
	if !updated.Permissions.Upload || !updated.Permissions.Delete || updated.MaxUploadBytes != 16 {
		t.Fatalf("updated share = %+v", updated)
	}

	wrongOwnerReq := newShareManagementRequestForUser("other", http.MethodPatch, "/api/v1/shares/"+share.ID, bytes.NewReader(updatedBody))
	wrongOwnerRec := httptest.NewRecorder()
	router.ServeHTTP(wrongOwnerRec, wrongOwnerReq)
	if wrongOwnerRec.Code != http.StatusNotFound {
		t.Fatalf("other owner's update status = %d, want 404", wrongOwnerRec.Code)
	}

	deleteReq := httptest.NewRequest(http.MethodDelete, deletePath, nil)
	deleteRec := httptest.NewRecorder()
	router.ServeHTTP(deleteRec, deleteReq)
	if deleteRec.Code != http.StatusOK {
		t.Fatalf("authorized delete status = %d, want 200: %s", deleteRec.Code, deleteRec.Body.String())
	}
	if exists, err := fs.Exists("/data/media/shared/note.txt"); err != nil || exists {
		t.Fatalf("deleted item remains (exists=%t, err=%v)", exists, err)
	}
}

func TestShareFolderArchiveViaAPI(t *testing.T) {
	handler, fs, _ := setupTestShareHandler()
	router := createShareTestRouter(handler)
	if err := fs.MkdirAll("/data/media/shared/nested", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := fs.WriteFile("/data/media/shared/nested/note.txt", []byte("archive note"), 0o644); err != nil {
		t.Fatal(err)
	}
	share := createShareViaAPI(t, router, "media/shared", model.SharePermissions{}, nil)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/share/"+share.Token+"/archive", nil)
	rec := httptest.NewRecorder()
	router.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("archive status = %d, want 200: %s", rec.Code, rec.Body.String())
	}
	if got := rec.Header().Get("Content-Type"); got != "application/zip" {
		t.Fatalf("archive Content-Type = %q, want application/zip", got)
	}
	if got := rec.Header().Get("Content-Disposition"); !strings.Contains(got, "shared.zip") {
		t.Fatalf("archive Content-Disposition = %q", got)
	}
	reader, err := zip.NewReader(bytes.NewReader(rec.Body.Bytes()), int64(rec.Body.Len()))
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, entry := range reader.File {
		if entry.Name != "shared/nested/note.txt" {
			continue
		}
		file, err := entry.Open()
		if err != nil {
			t.Fatal(err)
		}
		content, readErr := io.ReadAll(file)
		closeErr := file.Close()
		if readErr != nil || closeErr != nil {
			t.Fatalf("read archive item: read=%v close=%v", readErr, closeErr)
		}
		if string(content) != "archive note" {
			t.Fatalf("archive content = %q", content)
		}
		found = true
	}
	if !found {
		t.Fatalf("ZIP is missing shared/nested/note.txt: %+v", reader.File)
	}
}
