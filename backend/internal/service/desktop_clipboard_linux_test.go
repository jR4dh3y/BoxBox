package service

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"
)

type fakeClipboardCommandRunner struct {
	paths     map[string]string
	responses map[string][]byte
	calls     []clipboardCommandCall
}

type clipboardCommandCall struct {
	executable string
	args       []string
	input      []byte
}

func (r *fakeClipboardCommandRunner) LookPath(name string) (string, error) {
	path, ok := r.paths[name]
	if !ok {
		return "", errors.New("not found")
	}
	return path, nil
}

func (r *fakeClipboardCommandRunner) Run(_ context.Context, executable string, args []string, input []byte) ([]byte, error) {
	call := clipboardCommandCall{executable: executable, args: append([]string(nil), args...), input: append([]byte(nil), input...)}
	r.calls = append(r.calls, call)
	return append([]byte(nil), r.responses[strings.Join(args, " ")]...), nil
}

type fakeClipboardOwner struct {
	available bool
	formats   map[string][]byte
	called    bool
}

func (o *fakeClipboardOwner) Available(context.Context) bool {
	return o.available
}

func (o *fakeClipboardOwner) Set(_ context.Context, formats map[string][]byte) error {
	o.called = true
	o.formats = cloneClipboardFormats(formats)
	return nil
}

func TestLinuxClipboardAdapterReadsWaylandFileFormats(t *testing.T) {
	runner := &fakeClipboardCommandRunner{
		paths: map[string]string{"wl-paste": "/test/wl-paste"},
		responses: map[string][]byte{
			"--list-types": []byte(gnomeCopiedFilesMIME + "\n" + uriListMIME + "\n" + kdeCutSelectionMIME + "\n"),
			"--no-newline --type " + gnomeCopiedFilesMIME: []byte("cut\nfile:///home/me/item.txt\n"),
			"--no-newline --type " + uriListMIME:          []byte("file:///home/me/item.txt\r\n"),
			"--no-newline --type " + kdeCutSelectionMIME:  []byte("1"),
		},
	}
	adapter := &linuxClipboardAdapter{runner: runner, owner: &fakeClipboardOwner{}, env: func(key string) string {
		if key == "WAYLAND_DISPLAY" {
			return "wayland-1"
		}
		return ""
	}}
	formats, err := adapter.Read(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if string(formats[gnomeCopiedFilesMIME]) != "cut\nfile:///home/me/item.txt\n" || string(formats[kdeCutSelectionMIME]) != "1" {
		t.Fatalf("formats = %#v", formats)
	}
	if len(runner.calls) != 4 {
		t.Fatalf("command calls = %d, want TARGETS plus three MIME reads", len(runner.calls))
	}
	if runner.calls[0].executable != "/test/wl-paste" || !reflect.DeepEqual(runner.calls[0].args, []string{"--list-types"}) {
		t.Fatalf("target listing call = %#v", runner.calls[0])
	}
}

func TestLinuxClipboardAdapterPublishesAllFormatsThroughGTKOwner(t *testing.T) {
	runner := &fakeClipboardCommandRunner{}
	owner := &fakeClipboardOwner{available: true}
	adapter := &linuxClipboardAdapter{runner: runner, owner: owner, env: func(string) string { return "" }}
	formats := map[string][]byte{
		gnomeCopiedFilesMIME: []byte("cut\nfile:///home/me/item.txt\n"),
		uriListMIME:          []byte("file:///home/me/item.txt\r\n"),
		kdeCutSelectionMIME:  []byte("1"),
	}
	if err := adapter.Write(context.Background(), DesktopClipboardCut, formats); err != nil {
		t.Fatal(err)
	}
	if !owner.called || !reflect.DeepEqual(owner.formats, formats) {
		t.Fatalf("GTK owner got %#v", owner.formats)
	}
	if len(runner.calls) != 0 {
		t.Fatalf("unexpected command fallback: %#v", runner.calls)
	}
}

func TestLinuxClipboardAdapterFallbackIsCopyOnlyAndReportsMissingCutSupport(t *testing.T) {
	runner := &fakeClipboardCommandRunner{
		paths: map[string]string{"wl-copy": "/test/wl-copy"},
	}
	adapter := &linuxClipboardAdapter{runner: runner, owner: &fakeClipboardOwner{}, env: func(string) string { return "" }}
	uriList := []byte("file:///home/me/item.txt\r\n")
	if err := adapter.Write(context.Background(), DesktopClipboardCopy, map[string][]byte{uriListMIME: uriList}); err != nil {
		t.Fatal(err)
	}
	if len(runner.calls) != 1 || runner.calls[0].executable != "/test/wl-copy" ||
		!reflect.DeepEqual(runner.calls[0].args, []string{"--type", uriListMIME}) || !bytes.Equal(runner.calls[0].input, uriList) {
		t.Fatalf("copy process call = %#v", runner.calls)
	}

	runner.calls = nil
	err := adapter.Write(context.Background(), DesktopClipboardCut, map[string][]byte{uriListMIME: uriList})
	if !errors.Is(err, ErrDesktopClipboardUnavailable) {
		t.Fatalf("cut fallback error = %v", err)
	}
	if len(runner.calls) != 0 {
		t.Fatalf("cut unexpectedly degraded to a copy-only clipboard: %#v", runner.calls)
	}
}

func TestLinuxClipboardAdapterClearsViaWaylandUtility(t *testing.T) {
	runner := &fakeClipboardCommandRunner{paths: map[string]string{"wl-copy": "/test/wl-copy"}}
	adapter := &linuxClipboardAdapter{runner: runner, owner: &fakeClipboardOwner{}, env: func(string) string { return "" }}
	if err := adapter.Clear(context.Background()); err != nil {
		t.Fatal(err)
	}
	if len(runner.calls) != 1 || !reflect.DeepEqual(runner.calls[0].args, []string{"--clear"}) {
		t.Fatalf("clear process call = %#v", runner.calls)
	}
}

func TestSystemClipboardCommandRunnerUsesDirectProcessAndBoundsOutput(t *testing.T) {
	runner := systemClipboardCommandRunner{}
	input := []byte("file:///tmp/a%20b\r\n")
	output, err := runner.Run(context.Background(), "/bin/cat", nil, input)
	if err != nil || !bytes.Equal(output, input) {
		t.Fatalf("process result = %q, %v", output, err)
	}

	oversizedOutput, err := runner.Run(context.Background(), "/usr/bin/head", []string{"-c", "1048577", "/dev/zero"}, nil)
	if !errors.Is(err, errClipboardPayloadTooLarge) {
		t.Fatalf("oversize process error = %v (captured %d bytes, exceeded=%t)", err, len(oversizedOutput), err == errClipboardPayloadTooLarge)
	}
}

func TestGTKClipboardOwnerRestartsAfterHelperExit(t *testing.T) {
	stateFile := filepath.Join(t.TempDir(), "runs")
	helper := createGTKOwnerTestHelper(t, fmt.Sprintf(`#!/bin/sh
count=0
if [ -f %q ]; then count=$(cat %q); fi
count=$((count + 1))
printf '%%s' "$count" > %q
if [ "$count" -eq 1 ]; then exit 0; fi
while IFS= read -r line; do
	id=$(printf '%%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
	printf '{"id":%%s}\n' "$id"
done
`, stateFile, stateFile, stateFile))
	owner := newGTKClipboardOwner(systemClipboardCommandRunner{})
	owner.python = helper
	t.Cleanup(stopGTKClipboardOwner(t, owner))

	if err := owner.Set(context.Background(), map[string][]byte{uriListMIME: []byte("file:///tmp/item\r\n")}); !errors.Is(err, ErrDesktopClipboardUnavailable) {
		t.Fatalf("first helper exit error = %v", err)
	}
	if err := owner.Set(context.Background(), map[string][]byte{uriListMIME: []byte("file:///tmp/item\r\n")}); err != nil {
		t.Fatalf("second helper request = %v", err)
	}
	if got, err := os.ReadFile(stateFile); err != nil || string(got) != "2" {
		t.Fatalf("helper starts = %q, %v; want 2", got, err)
	}
}

func TestGTKClipboardOwnerStopsHelperWhenRequestTimesOut(t *testing.T) {
	helper := createGTKOwnerTestHelper(t, `#!/bin/sh
IFS= read -r request
IFS= read -r until_closed
`)
	owner := newGTKClipboardOwner(systemClipboardCommandRunner{})
	owner.python = helper
	ctx, cancel := context.WithTimeout(context.Background(), 250*time.Millisecond)
	defer cancel()

	err := owner.Set(ctx, map[string][]byte{uriListMIME: []byte("file:///tmp/item\r\n")})
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("timed out helper request = %v", err)
	}
	if owner.stdin != nil || owner.stdinPipe != nil || owner.command != nil || owner.done != nil {
		t.Fatal("timed-out helper process was left attached to the owner")
	}
}

func createGTKOwnerTestHelper(t *testing.T, script string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "python3")
	if err := os.WriteFile(path, []byte(script), 0700); err != nil {
		t.Fatal(err)
	}
	return path
}

func stopGTKClipboardOwner(t *testing.T, owner *gtkClipboardOwner) func() {
	t.Helper()
	return func() {
		owner.processMu.Lock()
		defer owner.processMu.Unlock()
		owner.stop()
	}
}
