package service

import (
	"bytes"
	"context"
	"errors"
	"image"
	"image/png"
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
	output        []byte
	outputError   error
	runOutput     []byte
	runError      error
	startError    error
	paths         map[string]string
	outputs       []desktopCommandCall
	runs          []desktopCommandCall
	starts        []desktopCommandCall
	defaultID     string
	ignoreDefault bool
}

func (r *fakeDesktopCommandRunner) Output(_ context.Context, name string, args ...string) ([]byte, error) {
	r.outputs = append(r.outputs, desktopCommandCall{name: name, args: append([]string(nil), args...)})
	if name == "gio" && len(args) == 2 && args[0] == "mime" {
		return []byte("Default application for text/plain: " + r.defaultID + "\n"), r.outputError
	}
	return r.output, r.outputError
}

func (r *fakeDesktopCommandRunner) Run(_ context.Context, name string, args ...string) ([]byte, error) {
	r.runs = append(r.runs, desktopCommandCall{name: name, args: append([]string(nil), args...)})
	if name == "gio" && len(args) == 3 && args[0] == "mime" && !r.ignoreDefault {
		r.defaultID = args[2]
	}
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
	if got := desktopAppIDs(listed.Recommended); !reflect.DeepEqual(got, wantIDs) {
		t.Fatalf("recommended IDs = %v, want %v", got, wantIDs)
	}
	if listed.MIMEType != "text/plain" || listed.Recommended[0].Name != "Visual Studio Code" || !listed.Recommended[0].IsDefault {
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

func desktopAppIDs(apps []DesktopApplication) []string {
	ids := make([]string, len(apps))
	for i, app := range apps {
		ids[i] = app.ID
	}
	return ids
}

func writeDesktopEntry(t *testing.T, name, body string) {
	t.Helper()
	if err := os.WriteFile(filepath.Join(xdgApplications(t), name), []byte("[Desktop Entry]\nType=Application\n"+body), 0600); err != nil {
		t.Fatal(err)
	}
}

func TestDesktopAppsOfferEveryLaunchableAppBeyondAssociations(t *testing.T) {
	root, filePath := setupDesktopAppFixture(t)
	writeDesktopEntry(t, "zip-viewer.desktop", "Name=Archive Viewer\nExec=viewer %U\nMimeType=application/zip;\n")
	writeDesktopEntry(t, "calculator.desktop", "Name=Calculator\nExec=calculator\n")
	writeDesktopEntry(t, "broken.desktop", "Name=Broken\nExec=\"broken %f\n")
	writeDesktopEntry(t, "handler.desktop", "Name=Handler Only\nNoDisplay=true\nExec=handler %f\n")
	writeDesktopEntry(t, "slides-handler.desktop", "Name=Slides Handler\nNoDisplay=true\nExec=slides %f\nMimeType=application/vnd.openxmlformats-officedocument.presentationml.presentation;\n")
	runner := &fakeDesktopCommandRunner{output: []byte("standard::content-type: application/vnd.openxmlformats-officedocument.presentationml.presentation\n")}
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)
	virtualPath := "home/My Files/annual report.txt"

	listed, err := apps.ListApps(context.Background(), virtualPath)
	if err != nil {
		t.Fatal(err)
	}
	if got := desktopAppIDs(listed.Recommended); !reflect.DeepEqual(got, []string{"slides-handler.desktop"}) {
		t.Fatalf("recommended IDs = %v", got)
	}
	// Removed associations only apply to the file type; the app stays launchable from the full list.
	wantOther := []string{"zip-viewer.desktop", "hidden.desktop", "simple-editor.desktop", "Visual Studio Code.desktop"}
	if got := desktopAppIDs(listed.Other); !reflect.DeepEqual(got, wantOther) {
		t.Fatalf("other IDs = %v, want %v", got, wantOther)
	}

	if err := apps.OpenWith(context.Background(), virtualPath, "zip-viewer.desktop"); err != nil {
		t.Fatal(err)
	}
	want := desktopCommandCall{name: "gio", args: []string{"launch", filepath.Join(xdgApplications(t), "zip-viewer.desktop"), filePath}}
	if len(runner.runs) != 1 || !reflect.DeepEqual(runner.runs[0], want) {
		t.Fatalf("runs = %#v, want %#v", runner.runs, want)
	}
	for _, id := range []string{"calculator.desktop", "broken.desktop", "handler.desktop", "missing.desktop", "../zip-viewer.desktop"} {
		if err := apps.OpenWith(context.Background(), virtualPath, id); !errors.Is(err, ErrDesktopApplicationUnavailable) {
			t.Errorf("OpenWith(%q) error = %v, want unavailable", id, err)
		}
	}
	if len(runner.runs) != 1 {
		t.Fatalf("unlisted applications launched: %#v", runner.runs)
	}
}

func TestDesktopAppsOfferAssociatedHandlerOnlyEntriesAndSkipStaleDefaults(t *testing.T) {
	root, _ := setupDesktopAppFixture(t)
	writeDesktopEntry(t, "userapp-browser.desktop", "Name=Browser\nNoDisplay=true\nExec=browser %u\n")
	if err := os.WriteFile(filepath.Join(os.Getenv("XDG_CONFIG_HOME"), "mimeapps.list"), []byte(`[Default Applications]
text/plain=uninstalled-office.desktop;userapp-browser.desktop;
`), 0600); err != nil {
		t.Fatal(err)
	}
	runner := &fakeDesktopCommandRunner{output: []byte("standard::content-type: text/plain\n")}
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)

	listed, err := apps.ListApps(context.Background(), "home/My Files/annual report.txt")
	if err != nil {
		t.Fatal(err)
	}
	if len(listed.Recommended) == 0 || listed.Recommended[0].ID != "userapp-browser.desktop" || !listed.Recommended[0].IsDefault {
		t.Fatalf("recommended = %#v, want the installed default first", listed.Recommended)
	}
	for _, app := range append(listed.Recommended, listed.Other...) {
		if app.ID == "uninstalled-office.desktop" {
			t.Fatalf("stale default was offered: %#v", listed)
		}
	}
}

func TestDesktopExecValidation(t *testing.T) {
	for exec, valid := range map[string]bool{
		`/usr/bin/code --reuse-window %F`:  true,
		`"/opt/My App/app" %u`:             true,
		`sh -c 'echo "$1"' sh %f`:          true,
		`app --title="Quote \" inside" %f`: true,
		`"/opt/unterminated %f`:            false,
		`app 'unterminated %f`:             false,
		`app trailing\`:                    false,
		`   `:                              false,
	} {
		if got := validDesktopExec(exec); got != valid {
			t.Errorf("validDesktopExec(%q) = %v, want %v", exec, got, valid)
		}
	}
	for exec, accepts := range map[string]bool{"app %F": true, "app %U": true, "app": false, "app %%f": false, "app %i %c": false} {
		if got := execAcceptsFiles(exec); got != accepts {
			t.Errorf("execAcceptsFiles(%q) = %v, want %v", exec, got, accepts)
		}
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

func TestDesktopSetDefaultValidatesFileAndInstalledApplication(t *testing.T) {
	root, _ := setupDesktopAppFixture(t)
	writeDesktopEntry(t, "other.desktop", "Name=Other\nExec=viewer %f\n")
	runner := &fakeDesktopCommandRunner{output: []byte("standard::content-type: text/plain\n")}
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)
	for _, id := range []string{"simple-editor.desktop", "other.desktop"} {
		if err := apps.SetDefault(context.Background(), "home/My Files/annual report.txt", id); err != nil {
			t.Fatal(err)
		}
		want := desktopCommandCall{name: "gio", args: []string{"mime", "text/plain", id}}
		if !reflect.DeepEqual(runner.runs[len(runner.runs)-1], want) {
			t.Fatalf("runs = %#v", runner.runs)
		}
	}
	for _, request := range [][2]string{{"home/../secret", "other.desktop"}, {"home/My Files", "other.desktop"}, {"home/My Files/annual report.txt", "../other.desktop"}, {"home/My Files/annual report.txt", "missing.desktop"}} {
		before := len(runner.runs)
		if err := apps.SetDefault(context.Background(), request[0], request[1]); err == nil || len(runner.runs) != before {
			t.Fatalf("accepted %#v", request)
		}
	}
	runner.runError = errors.New("read-only mimeapps.list")
	if err := apps.SetDefault(context.Background(), "home/My Files/annual report.txt", "other.desktop"); !errors.Is(err, ErrDesktopDefaultFailed) {
		t.Fatalf("error = %v", err)
	}
	runner.runError = nil
	runner.ignoreDefault = true
	runner.defaultID = "simple-editor.desktop"
	if err := apps.SetDefault(context.Background(), "home/My Files/annual report.txt", "other.desktop"); !errors.Is(err, ErrDesktopDefaultFailed) {
		t.Fatalf("unconfirmed default error = %v", err)
	}
}

func TestDesktopDefaultPreservesSpecificConfigAndSymlink(t *testing.T) {
	setupDesktopAppFixture(t)
	t.Setenv("XDG_CURRENT_DESKTOP", "First:Example")
	configHome := os.Getenv("XDG_CONFIG_HOME")
	target := filepath.Join(t.TempDir(), "mimeapps.list")
	original := "# Keep my settings\n[Default Applications]\ntext/plain=simple-editor.desktop;Visual Studio Code.desktop;\nimage/png=image.desktop;\n\n[Added Associations]\ntext/plain=other.desktop;\n"
	if err := os.WriteFile(target, []byte(original), 0640); err != nil {
		t.Fatal(err)
	}
	link := filepath.Join(configHome, "example-mimeapps.list")
	if err := os.Symlink(target, link); err != nil {
		t.Fatal(err)
	}
	if err := updateDesktopMIMEOverride("text/plain", "Visual Studio Code.desktop"); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(target)
	want := strings.Replace(original, "text/plain=simple-editor.desktop;Visual Studio Code.desktop;", "text/plain=Visual Studio Code.desktop;simple-editor.desktop;", 1)
	if err != nil || string(data) != want {
		t.Fatalf("config = %q, error = %v", data, err)
	}
	info, err := os.Lstat(link)
	if err != nil || info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("link was replaced: %v, %v", info, err)
	}
	info, err = os.Stat(target)
	if err != nil || info.Mode().Perm() != 0640 {
		t.Fatalf("mode changed: %v, %v", info, err)
	}
}

func TestDesktopIconsOnlyReturnBoundedRasterForInstalledEntries(t *testing.T) {
	root, _ := setupDesktopAppFixture(t)
	var buffer bytes.Buffer
	if err := png.Encode(&buffer, image.NewRGBA(image.Rect(0, 0, 32, 32))); err != nil {
		t.Fatal(err)
	}
	runner := &fakeDesktopCommandRunner{output: buffer.Bytes()}
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)
	icon, err := apps.Icon(context.Background(), "Visual Studio Code.desktop")
	if err != nil || !bytes.Equal(icon.PNG, buffer.Bytes()) {
		t.Fatalf("icon/error = %v/%v", icon, err)
	}
	call := runner.outputs[0]
	if call.name != "python3" || call.args[0] != "-I" || call.args[len(call.args)-1] != "code" {
		t.Fatalf("call = %#v", call)
	}
	if cached, err := apps.Icon(context.Background(), "Visual Studio Code.desktop"); err != nil || !bytes.Equal(cached.PNG, buffer.Bytes()) {
		t.Fatalf("cached icon/error = %v/%v", cached, err)
	}
	if len(runner.outputs) != 1 {
		t.Fatalf("icon helper calls = %d, want 1", len(runner.outputs))
	}
	for _, id := range []string{"../secret", "unknown.desktop", "simple-editor.desktop"} {
		if _, err := apps.Icon(context.Background(), id); !errors.Is(err, ErrDesktopApplicationUnavailable) {
			t.Fatalf("accepted %q: %v", id, err)
		}
	}
	runner.output = []byte("<svg onload='bad' />")
	apps = NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)
	if _, err := apps.Icon(context.Background(), "Visual Studio Code.desktop"); !errors.Is(err, ErrDesktopApplicationUnavailable) {
		t.Fatalf("accepted non-PNG: %v", err)
	}
}

func TestDesktopIconsNativeRasterization(t *testing.T) {
	root, _ := setupDesktopAppFixture(t)
	// GdkPixbuf discovers SVG loaders through the system XDG data directories.
	t.Setenv("XDG_DATA_DIRS", "/usr/local/share:/usr/share")
	commands := osDesktopCommandRunner{}
	if _, err := commands.Output(context.Background(), "python3", "-I", "-c", "import gi; gi.require_version('Gtk', '3.0'); from gi.repository import Gtk"); err != nil {
		t.Skip("optional Python GTK icon helper is unavailable")
	}
	iconPath := filepath.Join(t.TempDir(), "application.svg")
	if err := os.WriteFile(iconPath, []byte(`<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="#369"/></svg>`), 0600); err != nil {
		t.Fatal(err)
	}
	writeDesktopEntry(t, "native-icon.desktop", "Name=Native Icon\nExec=viewer %f\nIcon="+iconPath+"\n")
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, commands)
	icon, err := apps.Icon(context.Background(), "native-icon.desktop")
	if err != nil {
		t.Fatal(err)
	}
	config, err := png.DecodeConfig(bytes.NewReader(icon.PNG))
	if err != nil || config.Width != 32 || config.Height != 32 {
		t.Fatalf("icon = %#v, error = %v", config, err)
	}
}

func TestDesktopDefaultUsesDesktopSpecificAssociationAndFirstAvailableFallback(t *testing.T) {
	root, _ := setupDesktopAppFixture(t)
	t.Setenv("XDG_CURRENT_DESKTOP", "Example")
	if err := os.WriteFile(filepath.Join(os.Getenv("XDG_CONFIG_HOME"), "example-mimeapps.list"), []byte("[Default Applications]\ntext/plain=missing.desktop;simple-editor.desktop;Visual Studio Code.desktop;\n"), 0600); err != nil {
		t.Fatal(err)
	}
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, &fakeDesktopCommandRunner{output: []byte("standard::content-type: text/plain\n")})
	result, err := apps.ListApps(context.Background(), "home/My Files/annual report.txt")
	if err != nil {
		t.Fatal(err)
	}
	if result.Recommended[0].ID != "simple-editor.desktop" || !result.Recommended[0].IsDefault || result.Recommended[1].IsDefault {
		t.Fatalf("recommended = %#v", result.Recommended)
	}
}

func TestDesktopOpenConfinesPaths(t *testing.T) {
	root := t.TempDir()
	file := filepath.Join(root, "hello.txt")
	if err := os.WriteFile(file, []byte("hello"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(t.TempDir(), filepath.Join(root, "escape")); err != nil {
		t.Fatal(err)
	}
	runner := &fakeDesktopCommandRunner{}
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)
	if err := apps.Open(context.Background(), "home/hello.txt"); err != nil {
		t.Fatal(err)
	}
	want := []desktopCommandCall{{name: "xdg-open", args: []string{file}}}
	if !reflect.DeepEqual(runner.runs, want) {
		t.Fatalf("runs = %#v, want %#v", runner.runs, want)
	}
	for _, path := range []string{"home/../secret", "home/escape", "other/hello.txt", "home/missing", ""} {
		runner.runs = nil
		if err := apps.Open(context.Background(), path); err == nil {
			t.Fatalf("accepted %q", path)
		}
		if len(runner.runs) != 0 {
			t.Fatalf("launched opener for %q", path)
		}
	}
}

func TestDesktopAppsFollowEntrySymlinksAndRequireFileSupport(t *testing.T) {
	root, filePath := setupDesktopAppFixture(t)
	writeDesktopEntry(t, "unusable.desktop", "Name=No File Support\nExec=calculator\nMimeType=text/plain;\n")
	writeDesktopEntry(t, "dbus.desktop", "Name=Activated Editor\nDBusActivatable=true\nMimeType=text/plain;\n")
	entry := filepath.Join(t.TempDir(), "editor.desktop")
	if err := os.WriteFile(entry, []byte("[Desktop Entry]\nType=Application\nName=Linked Editor\nExec=editor %F\n"), 0600); err != nil {
		t.Fatal(err)
	}
	link := filepath.Join(xdgApplications(t), "linked.desktop")
	if err := os.Symlink(entry, link); err != nil {
		t.Fatal(err)
	}
	runner := &fakeDesktopCommandRunner{output: []byte("standard::content-type: text/plain\n")}
	apps := NewDesktopAppsService(filesystem.NewOsFS(), []model.MountPoint{{Name: "home", Path: root}}, runner)
	path := "home/My Files/annual report.txt"
	listed, err := apps.ListApps(context.Background(), path)
	if err != nil {
		t.Fatal(err)
	}
	found := make(map[string]bool)
	for _, app := range append(listed.Recommended, listed.Other...) {
		found[app.ID] = true
	}
	if found["unusable.desktop"] || !found["dbus.desktop"] || !found["linked.desktop"] {
		t.Fatalf("offered app IDs = %v", found)
	}
	if err := apps.OpenWith(context.Background(), path, "unusable.desktop"); !errors.Is(err, ErrDesktopApplicationUnavailable) {
		t.Fatalf("non-file handler error = %v", err)
	}
	if err := apps.OpenWith(context.Background(), path, "linked.desktop"); err != nil {
		t.Fatal(err)
	}
	want := desktopCommandCall{name: "gio", args: []string{"launch", link, filePath}}
	if len(runner.runs) != 1 || !reflect.DeepEqual(runner.runs[0], want) {
		t.Fatalf("runs = %#v, want %#v", runner.runs, want)
	}
}
