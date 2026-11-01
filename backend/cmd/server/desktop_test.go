package main

import (
	"encoding/hex"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

func TestDesktopHandler(t *testing.T) {
	token := strings.Repeat("a", 64)
	address := "127.0.0.1:12345"
	handler := desktopHandler(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusNoContent)
	}), address, token)
	for _, tc := range []struct {
		name, path, cookie, host, origin string
		status                           int
	}{
		{name: "no cookie", path: "/api/v1/files", status: 401},
		{name: "wrong cookie", path: "/api/v1/files", cookie: "wrong", status: 401},
		{name: "bootstrap rejected", path: "/_desktop?token=wrong", status: 401},
		{name: "bootstrap", path: "/_desktop?token=" + token, status: 303},
		{name: "authenticated", path: "/api/v1/files", cookie: token, status: 204},
		{name: "static protected", path: "/", status: 401},
		{name: "websocket protected", path: "/api/v1/ws", status: 401},
		{name: "websocket authenticated", path: "/api/v1/ws", cookie: token, origin: "http://" + address, status: 204},
		{name: "cross origin", path: "/api/v1/files", cookie: token, origin: "https://evil.example", status: 403},
		{name: "rebound host", path: "/api/v1/files", cookie: token, host: "evil.example:12345", status: 403},
	} {
		t.Run(tc.name, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodGet, "http://"+address+tc.path, nil)
			if tc.host != "" {
				req.Host = tc.host
			}
			if tc.origin != "" {
				req.Header.Set("Origin", tc.origin)
			}
			if tc.cookie != "" {
				req.AddCookie(&http.Cookie{Name: desktopCookie, Value: tc.cookie})
			}
			response := httptest.NewRecorder()
			handler.ServeHTTP(response, req)
			if response.Code != tc.status {
				t.Fatalf("status = %d, want %d", response.Code, tc.status)
			}
			if tc.status == 303 {
				cookies := response.Result().Cookies()
				if len(cookies) != 1 || !cookies[0].HttpOnly || cookies[0].SameSite != http.SameSiteStrictMode || cookies[0].Value != token {
					t.Fatal("bootstrap must set a private, strict session cookie")
				}
				if response.Header().Get("Location") != "/" {
					t.Fatal("bootstrap must remove token from URL")
				}
			}
		})
	}
}

func TestDesktopTokenRequired(t *testing.T) {
	for _, value := range []string{"", "short", strings.Repeat("a", 64)} {
		t.Setenv("BOXBOX_DESKTOP_TOKEN", value)
		_, err := desktopToken()
		if (err == nil) != (len(value) == 64) {
			t.Fatal("desktop token validation mismatch")
		}
	}
}

func TestDesktopBootstrapNavigationStaysLocal(t *testing.T) {
	token := strings.Repeat("a", 64)
	address := "127.0.0.1:12345"
	handler := desktopHandler(http.NotFoundHandler(), address, token)
	for _, tc := range []struct{ next, want string }{
		{"/browse?path=root%2Ftmp&select=root%2Ftmp%2Fexample.txt", "/browse?path=root%2Ftmp&select=root%2Ftmp%2Fexample.txt"},
		{"/chooser?id=request-123", "/chooser?id=request-123"},
		{"/%62rowse?path=root%2Ftmp", "/browse?path=root%2Ftmp"},
		{"/chooser?id=one%0d%0aLocation%3A%20https%3A%2F%2Foutside.example", "/chooser?id=one%0D%0ALocation%3A+https%3A%2F%2Foutside.example"},
		{"/browse/../outside", "/"},
		{"/browse%2f%2foutside.example", "/"},
		{"https://outside.example/browse", "/"},
		{"//outside.example/browse", "/"},
		{"/api/v1/desktop/open", "/"},
	} {
		r := httptest.NewRequest("GET", "http://"+address+"/_desktop?token="+token+"&next="+url.QueryEscape(tc.next), nil)
		w := httptest.NewRecorder()
		handler.ServeHTTP(w, r)
		if w.Code != 303 || w.Header().Get("Location") != tc.want {
			t.Fatalf("next=%q got status %d location %q", tc.next, w.Code, w.Header().Get("Location"))
		}
	}
}

func TestDesktopOpenLaunchesResolvedFile(t *testing.T) {
	dir := t.TempDir()
	file := filepath.Join(dir, "hello world.txt")
	capture := filepath.Join(dir, "opened")
	if err := os.WriteFile(file, []byte("hello"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "xdg-open"), []byte("#!/bin/sh\nprintf '%s' \"$1\" > \"$DESKTOP_OPEN_CAPTURE\"\n"), 0700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", dir)
	t.Setenv("DESKTOP_OPEN_CAPTURE", capture)
	cfg := &model.ServerConfig{MountPoints: []model.MountPoint{{Name: "home", Path: dir}}}
	routes := desktopRoutes(http.NotFoundHandler(), cfg)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/desktop/open", strings.NewReader(`{"path":"home/hello world.txt"}`))
	req.Header.Set("Content-Type", "application/json")
	response := httptest.NewRecorder()
	routes.ServeHTTP(response, req)
	if response.Code != http.StatusOK {
		t.Fatalf("status = %d", response.Code)
	}
	opened, err := os.ReadFile(capture)
	if err != nil {
		t.Fatal(err)
	}
	if string(opened) != file {
		t.Fatalf("opened %q, want %q", opened, file)
	}
}

func TestDesktopRemoteRoutesFailClosedAndKeepNativeRoutes(t *testing.T) {
	data := t.TempDir()
	// A corrupt store must not turn private API requests into a successful SPA response.
	if err := os.MkdirAll(filepath.Join(data, "desktop"), 0700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(data, "desktop", "servers.json"), []byte("corrupt"), 0600); err != nil {
		t.Fatal(err)
	}
	cfg := &model.ServerConfig{DataDir: data, MountPoints: []model.MountPoint{{Name: "test", Path: t.TempDir()}}}
	fallback := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html")
		w.Write([]byte("SPA"))
	})
	address := "127.0.0.1:12345"
	token := strings.Repeat("b", 64)
	router := desktopHandler(desktopRoutes(fallback, cfg), address, token)
	for _, path := range []string{"/api/v1/desktop/servers", "/api/v1/desktop/servers/id/api/v1/files", "/api/v1/desktop/transfers", "/api/v1/desktop/transfers/id"} {
		for _, cookie := range []bool{false, true} {
			r := httptest.NewRequest("GET", "http://"+address+path, nil)
			if cookie {
				r.AddCookie(&http.Cookie{Name: desktopCookie, Value: token})
			}
			w := httptest.NewRecorder()
			router.ServeHTTP(w, r)
			want := 401
			if cookie {
				want = 503
			}
			if w.Code != want {
				t.Fatalf("%s: %d want%d", path, w.Code, want)
			}
			if cookie && !strings.Contains(w.Header().Get("Content-Type"), "application/json") {
				t.Fatal("API failure fell through to SPA")
			}
		}
	}
	r := httptest.NewRequest("GET", "http://"+address+"/api/v1/desktop/trash", nil)
	r.AddCookie(&http.Cookie{Name: desktopCookie, Value: token})
	w := httptest.NewRecorder()
	router.ServeHTTP(w, r)
	if w.Code == 503 || strings.Contains(w.Body.String(), "SPA") {
		t.Fatalf("native route lost: %d", w.Code)
	}
}

func TestDesktopPreferencesGateAndNewOrigin(t *testing.T) {
	cfg := &model.ServerConfig{DataDir: t.TempDir()}
	token := strings.Repeat("p", 64)
	for index, address := range []string{"127.0.0.1:12345", "127.0.0.1:23456"} {
		router := desktopHandler(desktopRoutes(http.NotFoundHandler(), cfg), address, token)
		request := func(method, body, origin string, cookie bool) *httptest.ResponseRecorder {
			req := httptest.NewRequest(method, "http://"+address+"/api/v1/desktop/preferences", strings.NewReader(body))
			if origin != "" {
				req.Header.Set("Origin", origin)
			}
			if cookie {
				req.AddCookie(&http.Cookie{Name: desktopCookie, Value: token})
			}
			rec := httptest.NewRecorder()
			router.ServeHTTP(rec, req)
			return rec
		}
		if rec := request("GET", "", "", false); rec.Code != 401 {
			t.Fatalf("unguarded preferences: %d", rec.Code)
		}
		if rec := request("PUT", `{}`, "https://other.example", true); rec.Code != 403 {
			t.Fatalf("cross-origin preferences: %d", rec.Code)
		}
		if index == 0 {
			if rec := request("PUT", `{"compactMode":true,"favoriteFolders":[{"name":"Saved","path":"home/Saved"}]}`, "http://"+address, true); rec.Code != 204 {
				t.Fatalf("save preferences: %d %s", rec.Code, rec.Body.String())
			}
		} else {
			rec := request("GET", "", "http://"+address, true)
			if rec.Code != 200 || !strings.Contains(rec.Body.String(), `"compactMode":true`) || !strings.Contains(rec.Body.String(), "home/Saved") {
				t.Fatalf("preferences lost at new origin: %d %s", rec.Code, rec.Body.String())
			}
		}
	}
}

func TestDesktopRemoteLaunchUsesPrivateProfileAndBrowseBootstrap(t *testing.T) {
	cfg := &model.ServerConfig{DataDir: t.TempDir()}
	address := "127.0.0.1:12345"
	token := strings.Repeat("c", 64)
	router := desktopHandler(desktopRoutes(http.NotFoundHandler(), cfg), address, token)
	open := func(cookie bool) *httptest.ResponseRecorder {
		t.Helper()
		r := httptest.NewRequest("POST", "http://"+address+"/api/v1/desktop/servers/open", strings.NewReader(`{"url":"https://offline.example.test"}`))
		r.Header.Set("Content-Type", "application/json")
		if cookie {
			r.AddCookie(&http.Cookie{Name: desktopCookie, Value: token})
		}
		response := httptest.NewRecorder()
		router.ServeHTTP(response, r)
		return response
	}
	if response := open(false); response.Code != 401 {
		t.Fatalf("unauthenticated profile mutation: %d", response.Code)
	}
	response := open(true)
	var profile struct {
		ID string `json:"id"`
	}
	if response.Code != 200 || json.Unmarshal(response.Body.Bytes(), &profile) != nil || profile.ID == "" {
		t.Fatalf("native host profile request: %d", response.Code)
	}
	next := "/browse?path=" + url.QueryEscape("@remote/"+profile.ID)
	r := httptest.NewRequest("GET", "http://"+address+"/_desktop?token="+token+"&next="+url.QueryEscape(next), nil)
	bootstrap := httptest.NewRecorder()
	router.ServeHTTP(bootstrap, r)
	if bootstrap.Code != 303 || bootstrap.Header().Get("Location") != next {
		t.Fatal("remote bootstrap did not stay inside the local browse UI")
	}
	// The next process loads the same profile without a remote request or new ID.
	restarted := desktopHandler(desktopRoutes(http.NotFoundHandler(), cfg), address, token)
	list := httptest.NewRequest("GET", "http://"+address+"/api/v1/desktop/servers", nil)
	list.AddCookie(bootstrap.Result().Cookies()[0])
	saved := httptest.NewRecorder()
	restarted.ServeHTTP(saved, list)
	if saved.Code != 200 || !strings.Contains(saved.Body.String(), profile.ID) {
		t.Fatal("native launch profile did not survive backend restart")
	}
}

func TestDesktopWallpaperRecordsOutliveAProcessOnlyWhenSaved(t *testing.T) {
	cfg := &model.ServerConfig{DataDir: t.TempDir()}
	wallpapers := filepath.Join(cfg.DataDir, "wallpapers")
	address := "127.0.0.1:12345"
	token := strings.Repeat("w", 64)
	png := func(tag string) string { return "\x89PNG\r\n\x1a\n" + tag }
	var router http.Handler
	start := func() { router = desktopHandler(desktopRoutes(http.NotFoundHandler(), cfg), address, token) }
	send := func(method, path, body string) *httptest.ResponseRecorder {
		t.Helper()
		r := httptest.NewRequest(method, "http://"+address+"/api/v1/desktop"+path, strings.NewReader(body))
		r.AddCookie(&http.Cookie{Name: desktopCookie, Value: token})
		w := httptest.NewRecorder()
		router.ServeHTTP(w, r)
		return w
	}
	stage := func(image string) string {
		t.Helper()
		w := send("PUT", "/wallpaper", image)
		var meta model.WallpaperMeta
		if w.Code != 200 || json.Unmarshal(w.Body.Bytes(), &meta) != nil {
			t.Fatalf("stage: %d %s", w.Code, w.Body.String())
		}
		return "/api/v1/desktop/wallpaper/" + meta.SHA256
	}
	save := func(reference string) int {
		body := `{"backgroundImage":null}`
		if reference != "" {
			body = `{"backgroundImage":"` + reference + `"}`
		}
		return send("PUT", "/preferences", body).Code
	}
	served := func(reference string) bool {
		return send("GET", strings.TrimPrefix(reference, "/api/v1/desktop"), "").Code == 200
	}
	files := func() []string {
		entries, _ := os.ReadDir(wallpapers)
		names := make([]string, 0, len(entries))
		for _, entry := range entries {
			names = append(names, entry.Name())
		}
		return names
	}

	start()
	// Records that belong to accounts or other software are never selected.
	if err := os.MkdirAll(wallpapers, 0o755); err != nil {
		t.Fatal(err)
	}
	upper := strings.ToUpper(hex.EncodeToString([]byte("desktop-" + strings.Repeat("a", 64))))
	foreign := []string{"notes.txt", "desktop-x.json", "6465736b746f702d.json", "616c696365.json", "646576.json", upper + ".json"}
	for _, name := range foreign {
		if err := os.WriteFile(filepath.Join(wallpapers, name), []byte("{}"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	// Without a saved file the renderer may still adopt a fallback reference, so nothing is pruned.
	fallback := stage(png("fallback"))
	start()
	if !served(fallback) {
		t.Fatal("pruned before preferences were ever saved")
	}

	first := stage(png("first"))
	if save(first) != 204 {
		t.Fatal("save first")
	}
	// A replacement whose save is still pending, and same-image reuse of the saved image.
	second := stage(png("second"))
	if stage(png("first")) != first {
		t.Fatal("same image changed reference")
	}
	if save(second) != 204 || save(first) != 204 {
		t.Fatal("superseded saves")
	}
	// Staged records stay while this process serves a renderer that may still save them.
	if !served(second) || !served(fallback) {
		t.Fatal("pruned during the session")
	}
	// A rejected save does not change ownership.
	if send("PUT", "/preferences", `[]`).Code == 204 {
		t.Fatal("invalid preferences accepted")
	}
	// Interrupted writes of an abandoned stage.
	abandoned := stage(png("abandoned"))
	id := strings.TrimPrefix(abandoned, "/api/v1/desktop/wallpaper/")
	user := hex.EncodeToString([]byte("desktop-" + id))
	for _, name := range []string{user + ".json.tmp", user + "-" + id + ".img.tmp"} {
		if err := os.WriteFile(filepath.Join(wallpapers, name), []byte("partial"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.Remove(filepath.Join(wallpapers, user+".json")); err != nil {
		t.Fatal(err)
	}

	start()
	if !served(first) || served(second) || served(fallback) {
		t.Fatal("restart did not keep exactly the saved wallpaper")
	}
	want := []string{hex.EncodeToString([]byte("desktop-" + strings.TrimPrefix(first, "/api/v1/desktop/wallpaper/")))}
	for _, name := range files() {
		if strings.HasPrefix(name, want[0]) || slices.Contains(foreign, name) {
			continue
		}
		t.Fatalf("restart left %s", name)
	}
	if len(files()) != len(foreign)+2 {
		t.Fatalf("saved record or foreign files lost: %v", files())
	}

	// Removal releases the image at the next start.
	if save("") != 204 {
		t.Fatal("remove")
	}
	start()
	if served(first) || len(files()) != len(foreign) {
		t.Fatalf("removed wallpaper kept: %v", files())
	}

	// Unreadable preferences never prune.
	kept := stage(png("kept"))
	if err := os.WriteFile(filepath.Join(cfg.DataDir, "desktop", "preferences.json"), []byte("{"), 0o600); err != nil {
		t.Fatal(err)
	}
	start()
	if !served(kept) {
		t.Fatal("pruned with unreadable preferences")
	}
}
