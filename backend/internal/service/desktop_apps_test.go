package service

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

type desktopCommandCall struct {
	name string
	args []string
	dir  string
}

type fakeDesktopCommandRunner struct {
	output      []byte
	outputError error
	runOutput   []byte
	runError    error
	startError  error
	paths       map[string]string
	outputs     []desktopCommandCall
	runs        []desktopCommandCall
	starts      []desktopCommandCall
}

func (r *fakeDesktopCommandRunner) Output(_ context.Context, name string, args ...string) ([]byte, error) {
	r.outputs = append(r.outputs, desktopCommandCall{name: name, args: append([]string(nil), args...)})
	return r.output, r.outputError
}

func (r *fakeDesktopCommandRunner) Run(_ context.Context, name string, args ...string) ([]byte, error) {
	r.runs = append(r.runs, desktopCommandCall{name: name, args: append([]string(nil), args...)})
	return r.runOutput, r.runError
}

func (r *fakeDesktopCommandRunner) Start(name string, args []string, workingDirectory string) error {
	r.starts = append(r.starts, desktopCommandCall{
		name: name,
		args: append([]string(nil), args...),
		dir:  workingDirectory,
	})
	return r.startError
}

func (r *fakeDesktopCommandRunner) LookPath(name string) (string, error) {
	if path, ok := r.paths[name]; ok {
		return path, nil
	}
	return "", os.ErrNotExist
}

func TestDesktopAppsUseXDGAssociationsAndSafeGioLaunch(t *testing.T) {
	root, filePath := setupDesktopAppFixture(t)
	runner := &fakeDesktopCommandRunner{output: []byte("standard::content-type: text/plain\n")}
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)
	virtualPath := "home/My Files/annual report.txt"

	listed, err := apps.ListApps(context.Background(), virtualPath)
	if err != nil {
		t.Fatal(err)
	}
	wantIDs := []string{"Visual Studio Code.desktop", "simple-editor.desktop"}
	gotIDs := make([]string, len(listed.Apps))
	for i, app := range listed.Apps {
		gotIDs[i] = app.ID
	}
	if !reflect.DeepEqual(gotIDs, wantIDs) {
		t.Fatalf("application IDs = %v, want %v", gotIDs, wantIDs)
	}
	if listed.MIMEType != "text/plain" || listed.Apps[0].Name != "Visual Studio Code" || !listed.Apps[0].IsDefault {
		t.Fatalf("unexpected application list: %#v", listed)
	}

	if err := apps.OpenWith(context.Background(), virtualPath, "Visual Studio Code.desktop"); err != nil {
		t.Fatal(err)
	}
	if len(runner.runs) != 1 {
		t.Fatalf("run calls = %d, want 1", len(runner.runs))
	}
	call := runner.runs[0]
	if call.name != "gio" || !reflect.DeepEqual(call.args, []string{
		"launch",
		filepath.Join(xdgApplications(t), "Visual Studio Code.desktop"),
		filePath,
	}) {
		t.Fatalf("GIO launch call = %#v", call)
	}
	if len(runner.starts) != 0 || len(runner.outputs) != 2 {
		t.Fatalf("unexpected process calls: runs=%#v starts=%#v outputs=%#v", runner.runs, runner.starts, runner.outputs)
	}
}

func TestDesktopAppsPropagateGioLaunchFailure(t *testing.T) {
	root, _ := setupDesktopAppFixture(t)
	runner := &fakeDesktopCommandRunner{
		output:    []byte("standard::content-type: text/plain\n"),
		runOutput: []byte("gio: Failed to execute child process /missing/editor"),
		runError:  errors.New("exit status 1"),
	}
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)

	err := apps.OpenWith(context.Background(), "home/My Files/annual report.txt", "Visual Studio Code.desktop")
	if !errors.Is(err, ErrDesktopLaunchFailed) {
		t.Fatalf("OpenWith() error = %v, want ErrDesktopLaunchFailed", err)
	}
	if !strings.Contains(err.Error(), "Failed to execute child process /missing/editor") {
		t.Fatalf("launch error lost GIO's reason: %v", err)
	}
}

func TestDesktopAppsRejectUnassociatedClientAppAndEscapingPaths(t *testing.T) {
	root, _ := setupDesktopAppFixture(t)
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(root, "escape")); err != nil {
		t.Fatal(err)
	}
	runner := &fakeDesktopCommandRunner{output: []byte("standard::content-type: text/plain\n")}
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)

	if err := apps.OpenWith(context.Background(), "home/My Files/annual report.txt", "arbitrary.desktop"); !errors.Is(err, ErrDesktopApplicationUnavailable) {
		t.Fatalf("OpenWith(arbitrary app) error = %v", err)
	}
	for _, path := range []string{
		"home/../outside.txt",
		"home/escape/secret.txt",
		"other/file.txt",
	} {
		if _, err := apps.ListApps(context.Background(), path); err == nil {
			t.Errorf("ListApps(%q) unexpectedly succeeded", path)
		}
	}
	if len(runner.starts) != 0 || len(runner.outputs) != 1 {
		t.Fatalf("invalid requests reached process boundary: starts=%#v outputs=%#v", runner.starts, runner.outputs)
	}
}

func TestDesktopTerminalStartsInSelectedFileDirectoryWithArgv(t *testing.T) {
	root, filePath := setupDesktopAppFixture(t)
	terminalPath := filepath.Join(t.TempDir(), "kitty")
	runner := &fakeDesktopCommandRunner{paths: map[string]string{"kitty": terminalPath}}
	t.Setenv("TERMINAL", "kitty")
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)
	directory := filepath.Dir(filePath)

	if err := apps.OpenTerminal(context.Background(), "home/My Files/annual report.txt"); err != nil {
		t.Fatal(err)
	}
	if len(runner.starts) != 1 {
		t.Fatalf("start calls = %d, want 1", len(runner.starts))
	}
	call := runner.starts[0]
	if call.name != terminalPath || call.dir != directory || !reflect.DeepEqual(call.args, []string{"--directory", directory}) {
		t.Fatalf("terminal call = %#v", call)
	}
}

func TestDesktopTerminalReportsUnavailable(t *testing.T) {
	root, _ := setupDesktopAppFixture(t)
	runner := &fakeDesktopCommandRunner{paths: map[string]string{}}
	t.Setenv("TERMINAL", "")
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)
	if err := apps.OpenTerminal(context.Background(), "home/My Files"); !errors.Is(err, ErrDesktopTerminalUnavailable) {
		t.Fatalf("OpenTerminal() error = %v, want terminal unavailable", err)
	}
	if len(runner.starts) != 0 {
		t.Fatalf("unexpected terminal start: %#v", runner.starts)
	}
}

func TestDesktopTerminalLaunchFailureIsDistinct(t *testing.T) {
	root, _ := setupDesktopAppFixture(t)
	runner := &fakeDesktopCommandRunner{
		paths:      map[string]string{"kitty": "/fake/bin/kitty"},
		startError: errors.New("permission denied"),
	}
	t.Setenv("TERMINAL", "kitty")
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)
	if err := apps.OpenTerminal(context.Background(), "home/My Files"); !errors.Is(err, ErrDesktopTerminalLaunchFailed) {
		t.Fatalf("OpenTerminal() error = %v, want terminal launch failure", err)
	}
}

func setupDesktopAppFixture(t *testing.T) (string, string) {
	t.Helper()

	root := t.TempDir()
	directory := filepath.Join(root, "My Files")
	if err := os.MkdirAll(directory, 0755); err != nil {
		t.Fatal(err)
	}
	filePath := filepath.Join(directory, "annual report.txt")
	if err := os.WriteFile(filePath, []byte("report"), 0600); err != nil {
		t.Fatal(err)
	}

	xdgRoot := t.TempDir()
	dataHome := filepath.Join(xdgRoot, "data")
	configHome := filepath.Join(xdgRoot, "config")
	if err := os.MkdirAll(filepath.Join(dataHome, "applications"), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(configHome, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dataHome, "applications", "Visual Studio Code.desktop"), []byte(`[Desktop Entry]
Type=Application
Name=Visual Studio Code
Exec=/usr/bin/code --reuse-window %F
Icon=code
MimeType=text/plain;
`), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dataHome, "applications", "simple-editor.desktop"), []byte(`[Desktop Entry]
Type=Application
Name=Simple Editor
Exec=/usr/bin/editor %F
MimeType=text/plain;
`), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dataHome, "applications", "hidden.desktop"), []byte(`[Desktop Entry]
Type=Application
Name=Hidden
Exec=/usr/bin/hidden %F
MimeType=text/plain;
`), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(configHome, "mimeapps.list"), []byte(`[Default Applications]
text/plain=Visual Studio Code.desktop;

[Added Associations]
text/plain=simple-editor.desktop;hidden.desktop;

[Removed Associations]
text/plain=hidden.desktop;
`), 0600); err != nil {
		t.Fatal(err)
	}

	t.Setenv("HOME", filepath.Join(xdgRoot, "home"))
	t.Setenv("XDG_DATA_HOME", dataHome)
	t.Setenv("XDG_DATA_DIRS", filepath.Join(xdgRoot, "system-data"))
	t.Setenv("XDG_CONFIG_HOME", configHome)
	t.Setenv("XDG_CONFIG_DIRS", filepath.Join(xdgRoot, "system-config"))
	t.Setenv("TERMINAL", "")
	return root, filePath
}

func xdgApplications(t *testing.T) string {
	t.Helper()
	return filepath.Join(os.Getenv("XDG_DATA_HOME"), "applications")
}
