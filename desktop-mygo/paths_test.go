package main

import (
	"net/url"
	"os"
	"slices"
	"testing"
)

func TestLocalPath(t *testing.T) {
	s := &desktopServer{mounts: []mount{{Name: "root", Path: "/"}, {Name: "home", Path: "/home/u"}}}
	cases := []struct {
		virtual, local string
		ok             bool
	}{
		{"root", "/", true},
		{"root/etc/hosts", "/etc/hosts", true},
		{"home/a b/c.txt", "/home/u/a b/c.txt", true},
		{"home/x..y", "/home/u/x..y", true},
		{"home/../etc", "", false},
		{"home/a/../../etc", "", false},
		{"remote:server/a", "", false},
		{"music/a.mp3", "", false},
	}
	for _, c := range cases {
		local, ok := s.localPath(c.virtual)
		if local != c.local || ok != c.ok {
			t.Errorf("localPath(%q) = %q, %v; want %q, %v", c.virtual, local, ok, c.local, c.ok)
		}
	}
}

func TestParseURIList(t *testing.T) {
	got := parseURIList([]byte("# comment\r\nfile:///tmp/a%20b.txt\r\nhttps://example.com/x\r\nfile://localhost/etc/hosts\r\nfile://other/x\r\n\r\n"))
	want := []string{"/tmp/a b.txt", "/etc/hosts"}
	if !slices.Equal(got, want) {
		t.Errorf("parseURIList = %q, want %q", got, want)
	}
}

func TestFileURL(t *testing.T) {
	for path, want := range map[string]string{
		"/tmp/a b.txt": "file:///tmp/a%20b.txt",
		"/tmp/#1?.txt": "file:///tmp/%231%3F.txt",
	} {
		if got := fileURL(path); got != want {
			t.Errorf("fileURL(%q) = %q, want %q", path, got, want)
		}
		if got := parseURIList([]byte(fileURL(path))); !slices.Equal(got, []string{path}) {
			t.Errorf("round trip of %q = %q", path, got)
		}
	}
}

func TestBrowsePath(t *testing.T) {
	dir := t.TempDir()
	target, err := parseLaunchTarget(dir, "", "/")
	if err != nil {
		t.Fatal(err)
	}
	if got, want := browsePath(target), "/browse?path="+url.QueryEscape("root"+dir); got != want {
		t.Errorf("browsePath = %q, want %q", got, want)
	}
	if browsePath(nil) != "/browse" {
		t.Error("no target should open /browse")
	}
	if _, err := parseLaunchTarget("https://example.com/", "", "/"); err == nil {
		t.Error("an https URL must be rejected")
	}
}

func TestParseLaunch(t *testing.T) {
	dir := t.TempDir()
	file := dir + "/a.txt"
	if err := os.WriteFile(file, nil, 0o600); err != nil {
		t.Fatal(err)
	}
	opts, err := parseLaunch([]string{dir, "--data-dir", "/d", "--select=" + file}, "/", "")
	if err != nil {
		t.Fatal(err)
	}
	if opts.request.kind != requestLocal || opts.request.target != (launchTarget{Path: "root" + dir, Select: "root" + file}) || opts.dataDir != "/d" {
		t.Errorf("parseLaunch = %+v", opts)
	}
	if opts, _ := parseLaunch(nil, "/", ""); opts.request.kind != requestActivate {
		t.Errorf("no arguments = %+v, want activate", opts.request)
	}
	if opts, _ := parseLaunch([]string{"--portal"}, "/", "https://x.example"); opts.request.kind != requestPortal {
		t.Errorf("--portal = %+v", opts.request)
	}
	if opts, _ := parseLaunch([]string{dir}, "/", "https://files.example.com"); opts.request.kind != requestRemote || opts.request.remote != "https://files.example.com" {
		t.Errorf("a remote URL must win: %+v", opts.request)
	}
	for _, bad := range []string{"http://files.example.com", "https://u:p@x.example", "https://x.example/?q=1"} {
		if _, err := parseLaunch([]string{"--remote", bad}, "/", ""); err == nil {
			t.Errorf("--remote %s must be rejected", bad)
		}
	}
	if _, err := parseLaunch([]string{"--bogus"}, "/", ""); err == nil {
		t.Error("an unknown option must be rejected")
	}
}
