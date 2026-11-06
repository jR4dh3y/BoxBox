package service

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestChooserRemoteListingAndLocalHandoff(t *testing.T) {
	servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/box/api/v1/files/list/photos":
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(model.FileList{Path: "photos", Items: []model.FileInfo{{Name: "logo", Size: 8}}})
		case "/box/api/v1/stream/download/photos/logo":
			w.Header().Set("Content-Type", "image/png")
			w.Write([]byte("\x89PNG\r\n\x1a\n"))
		default:
			t.Errorf("unexpected upstream route %s", r.URL.Path)
			w.WriteHeader(404)
		}
	})
	chooser, _ := chooserFixture(t)
	cache := t.TempDir()
	chooser.ConfigureServers(servers, cache)
	request, result := chooser.Begin(ChooserRequest{Kind: "open", Filters: []ChooserFilter{{Name: "Images", Rules: []ChooserRule{{Kind: 1, Value: "image/png"}}}}})
	reference := "@remote/" + server.ID + "/photos"
	list, err := chooser.Directory(context.Background(), request.ID, reference)
	if err != nil {
		t.Fatal(err)
	}
	if list.Path != reference || len(list.Items) != 1 || list.Items[0].Path != reference+"/logo" || list.Items[0].MimeType != "image/png" {
		t.Fatalf("list %#v", list)
	}
	if err := chooser.Select(context.Background(), request.ID, ChooserSelection{Paths: []string{reference + "/logo"}}); err != nil {
		t.Fatal(err)
	}
	got := <-result
	if len(got.URIs) != 1 {
		t.Fatalf("result %#v", got)
	}
	uri, err := url.Parse(got.URIs[0])
	if err != nil {
		t.Fatal(err)
	}
	if uri.Scheme != "file" || !strings.HasPrefix(uri.Path, cache+string(filepath.Separator)) || filepath.Base(uri.Path) != "logo" {
		t.Fatalf("URI %s", got.URIs[0])
	}
	data, err := os.ReadFile(uri.Path)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "\x89PNG\r\n\x1a\n" {
		t.Fatalf("incomplete copy %q", data)
	}
	info, _ := os.Stat(uri.Path)
	if info.Mode().Perm() != 0600 {
		t.Fatalf("permissions %v", info.Mode())
	}
}
func TestChooserRemoteFailureRemovesBatchAndCanRetry(t *testing.T) {
	servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
		if strings.HasSuffix(r.URL.Path, "/missing.txt") {
			w.WriteHeader(404)
			return
		}
		io.WriteString(w, "text")
	})
	chooser, _ := chooserFixture(t)
	cache := t.TempDir()
	chooser.ConfigureServers(servers, cache)
	request, result := chooser.Begin(ChooserRequest{Kind: "open", Multiple: true})
	reference := "@remote/" + server.ID + "/photos/"
	err := chooser.Select(context.Background(), request.ID, ChooserSelection{Paths: []string{reference + "first.txt", reference + "missing.txt"}})
	if err == nil {
		t.Fatal("partial batch accepted")
	}
	entries, _ := os.ReadDir(cache)
	if len(entries) != 0 {
		t.Fatal("partial files retained")
	}
	select {
	case <-result:
		t.Fatal("failed import completed portal request")
	default:
	}
	if err := chooser.Select(context.Background(), request.ID, ChooserSelection{Paths: []string{reference + "first.txt"}}); err != nil {
		t.Fatal(err)
	}
	<-result
}
func TestChooserRemoteRejectsSaveDirectoryTraversalAndFilterMismatch(t *testing.T) {
	servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) { io.WriteString(w, "text") })
	chooser, _ := chooserFixture(t)
	cache := t.TempDir()
	chooser.ConfigureServers(servers, cache)
	for _, request := range []ChooserRequest{{Kind: "save"}, {Kind: "save-files", Files: []string{"a.txt"}}, {Kind: "open", Directory: true}, {Kind: "open", Filters: []ChooserFilter{{Rules: []ChooserRule{{Kind: 0, Value: "*.png"}}}}}} {
		req, _ := chooser.Begin(request)
		if err := chooser.Select(context.Background(), req.ID, ChooserSelection{Paths: []string{"@remote/" + server.ID + "/photos/file.txt"}}); err == nil {
			t.Fatalf("unsupported selection accepted %#v", request)
		}
		chooser.Cancel(req.ID)
	}
	req, _ := chooser.Begin(ChooserRequest{Kind: "open"})
	if err := chooser.Select(context.Background(), req.ID, ChooserSelection{Paths: []string{"@remote/" + server.ID + "/photos/../secret"}}); err == nil {
		t.Fatal("remote traversal accepted")
	}
	entries, _ := os.ReadDir(cache)
	if len(entries) != 0 {
		t.Fatal("rejected files retained")
	}
}
func TestChooserRemoteCancelStopsDownloadAndCleansPartialFiles(t *testing.T) {
	started := make(chan struct{})
	stopped := make(chan struct{})
	servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("partial"))
		w.(http.Flusher).Flush()
		close(started)
		<-r.Context().Done()
		close(stopped)
	})
	chooser, _ := chooserFixture(t)
	cache := t.TempDir()
	chooser.ConfigureServers(servers, cache)
	request, result := chooser.Begin(ChooserRequest{Kind: "open"})
	done := make(chan error, 1)
	go func() {
		done <- chooser.Select(context.Background(), request.ID, ChooserSelection{Paths: []string{"@remote/" + server.ID + "/photos/file.txt"}})
	}()
	select {
	case <-started:
	case <-time.After(5 * time.Second):
		t.Fatal("download did not start")
	}
	chooser.Cancel(request.ID)
	if got := <-result; got.Code != 1 {
		t.Fatalf("cancel result %#v", got)
	}
	select {
	case err := <-done:
		if err == nil {
			t.Fatal("cancelled import succeeded")
		}
	case <-time.After(5 * time.Second):
		t.Fatal("import did not cancel")
	}
	select {
	case <-stopped:
	case <-time.After(5 * time.Second):
		t.Fatal("upstream did not cancel")
	}
	entries, _ := os.ReadDir(cache)
	if len(entries) != 0 {
		t.Fatal("cancelled files retained")
	}
}

func TestChooserRemoteProbeFailureKeepsSiblingsAndRevalidatesContent(t *testing.T) {
	for _, status := range []int{404, 500, 200} {
		t.Run(fmt.Sprint(status), func(t *testing.T) {
			servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
				switch {
				case strings.Contains(r.URL.Path, "/files/list/"):
					w.Header().Set("Content-Type", "application/json")
					json.NewEncoder(w).Encode(model.FileList{Items: []model.FileInfo{{Name: "unreadable"}, {Name: "logo"}}})
				case strings.HasSuffix(r.URL.Path, "/unreadable"):
					if status == 200 {
						w.Header().Set("Content-Length", "100")
						io.WriteString(w, "partial")
					} else {
						w.WriteHeader(status)
					}
				case r.Header.Get("Range") != "":
					w.Write([]byte("\x89PNG\r\n\x1a\n"))
				default:
					io.WriteString(w, "content changed since listing")
				}
			})
			chooser, _ := chooserFixture(t)
			defer chooser.Close()
			cache := t.TempDir()
			chooser.ConfigureServers(servers, cache)
			request, _ := chooser.Begin(ChooserRequest{Kind: "open", Filters: []ChooserFilter{{Rules: []ChooserRule{{Kind: 1, Value: "image/png"}}}}})
			reference := "@remote/" + server.ID + "/photos"
			list, err := chooser.Directory(context.Background(), request.ID, reference)
			if err != nil {
				t.Fatal(err)
			}
			if len(list.Items) != 2 || list.Items[0].MimeType != "" || list.Items[1].MimeType != "image/png" {
				t.Fatalf("listing %#v", list)
			}
			if err := chooser.Select(context.Background(), request.ID, ChooserSelection{Paths: []string{reference + "/logo"}}); !errors.Is(err, ErrInvalidOperation) {
				t.Fatalf("content mismatch accepted: %v", err)
			}
			entries, err := os.ReadDir(cache)
			if err != nil || len(entries) != 0 {
				t.Fatalf("failed import retained files: %v %v", entries, err)
			}
		})
	}
}

func TestChooserRemotePercentNamesDoNotHideSiblings(t *testing.T) {
	servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
		if !strings.Contains(r.URL.Path, "/files/list/") {
			t.Errorf("unsupported name requested: %s", r.URL.Path)
			w.WriteHeader(404)
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(model.FileList{Items: []model.FileInfo{{Name: "100%.txt"}, {Name: "%2e%2e"}, {Name: "readable.txt"}}})
	})
	chooser, _ := chooserFixture(t)
	defer chooser.Close()
	chooser.ConfigureServers(servers, t.TempDir())
	request, _ := chooser.Begin(ChooserRequest{Kind: "open"})
	reference := "@remote/" + server.ID + "/photos"
	list, err := chooser.Directory(context.Background(), request.ID, reference)
	if err != nil {
		t.Fatal(err)
	}
	if len(list.Items) != 1 || list.Items[0].Path != reference+"/readable.txt" {
		t.Fatalf("listing %#v", list)
	}
	if err := chooser.Select(context.Background(), request.ID, ChooserSelection{Paths: []string{reference + "/100%.txt"}}); err == nil {
		t.Fatal("percent import accepted")
	}
}

func TestChooserRemoteRejectsMalformedListingNames(t *testing.T) {
	for _, name := range []string{"", ".", "..", "../secret", "/absolute", "sub/file", `sub\file`, "bad\x00name", "@remote"} {
		t.Run(name, func(t *testing.T) {
			servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
				w.Header().Set("Content-Type", "application/json")
				json.NewEncoder(w).Encode(model.FileList{Items: []model.FileInfo{{Name: name}, {Name: "safe.txt"}}})
			})
			chooser, _ := chooserFixture(t)
			defer chooser.Close()
			chooser.ConfigureServers(servers, t.TempDir())
			request, _ := chooser.Begin(ChooserRequest{Kind: "open"})
			if _, err := chooser.Directory(context.Background(), request.ID, "@remote/"+server.ID+"/photos"); !errors.Is(err, ErrRemoteUnavailable) {
				t.Fatalf("malformed name accepted: %v", err)
			}
		})
	}
}

func TestChooserRemoteProbePreservesCancellationAndAuthentication(t *testing.T) {
	for _, status := range []int{401, 403, 0} {
		t.Run(fmt.Sprint(status), func(t *testing.T) {
			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()
			servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
				if strings.Contains(r.URL.Path, "/files/list/") {
					w.Header().Set("Content-Type", "application/json")
					json.NewEncoder(w).Encode(model.FileList{Items: []model.FileInfo{{Name: "logo"}}})
					return
				}
				if status == 0 {
					cancel()
					<-r.Context().Done()
					return
				}
				w.WriteHeader(status)
			})
			chooser, _ := chooserFixture(t)
			defer chooser.Close()
			chooser.ConfigureServers(servers, t.TempDir())
			request, _ := chooser.Begin(ChooserRequest{Kind: "open", Filters: []ChooserFilter{{Rules: []ChooserRule{{Kind: 1, Value: "image/png"}}}}})
			_, err := chooser.Directory(ctx, request.ID, "@remote/"+server.ID+"/photos")
			expected := ErrRemoteAuthRequired
			if status == 0 {
				expected = context.Canceled
			}
			if !errors.Is(err, expected) {
				t.Fatalf("got %v, want %v", err, expected)
			}
		})
	}
}

func TestChooserRemoteListingFailureStillSurfaces(t *testing.T) {
	for _, status := range []int{401, 403, 500} {
		t.Run(fmt.Sprint(status), func(t *testing.T) {
			servers, server, _ := gatewayFixture(t, func(w http.ResponseWriter, r *http.Request) {
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(status)
				io.WriteString(w, `{}`)
			})
			chooser, _ := chooserFixture(t)
			defer chooser.Close()
			chooser.ConfigureServers(servers, t.TempDir())
			request, _ := chooser.Begin(ChooserRequest{Kind: "open"})
			_, err := chooser.Directory(context.Background(), request.ID, "@remote/"+server.ID+"/photos")
			expected := ErrRemoteAuthRequired
			if status == 500 {
				expected = ErrRemoteUnavailable
			}
			if !errors.Is(err, expected) {
				t.Fatalf("got %v, want %v", err, expected)
			}
		})
	}
}
