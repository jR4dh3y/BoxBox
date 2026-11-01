package service

import (
	"bufio"
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"
	"sync"
	"time"
)

var errClipboardPayloadTooLarge = errors.New("clipboard payload is too large")

type clipboardCommandRunner interface {
	LookPath(string) (string, error)
	Run(context.Context, string, []string, []byte) ([]byte, error)
}

type systemClipboardCommandRunner struct{}

func (systemClipboardCommandRunner) LookPath(name string) (string, error) {
	return exec.LookPath(name)
}

func (systemClipboardCommandRunner) Run(ctx context.Context, executable string, args []string, input []byte) ([]byte, error) {
	command := exec.CommandContext(ctx, executable, args...)
	if len(input) > 0 {
		command.Stdin = bytes.NewReader(input)
	}
	output := &limitedClipboardBuffer{limit: clipboardMaxPayloadSize}
	command.Stdout = output
	command.Stderr = io.Discard
	if err := command.Run(); err != nil {
		return output.Bytes(), err
	}
	if output.exceeded {
		return nil, errClipboardPayloadTooLarge
	}
	return output.Bytes(), nil
}

type limitedClipboardBuffer struct {
	data     []byte
	limit    int
	exceeded bool
}

func (b *limitedClipboardBuffer) Write(value []byte) (int, error) {
	remaining := b.limit - len(b.data)
	if remaining > 0 {
		if len(value) > remaining {
			b.data = append(b.data, value[:remaining]...)
		} else {
			b.data = append(b.data, value...)
		}
	}
	if len(value) > remaining {
		b.exceeded = true
	}
	return len(value), nil
}

func (b *limitedClipboardBuffer) Bytes() []byte {
	return b.data
}

type clipboardOwner interface {
	Available(context.Context) bool
	Set(context.Context, map[string][]byte) error
}

type linuxClipboardAdapter struct {
	runner clipboardCommandRunner
	owner  clipboardOwner
	env    func(string) string
}

func newLinuxClipboardAdapter() clipboardAdapter {
	runner := systemClipboardCommandRunner{}
	return &linuxClipboardAdapter{
		runner: runner,
		owner:  newGTKClipboardOwner(runner),
		env:    os.Getenv,
	}
}

func (a *linuxClipboardAdapter) Read(ctx context.Context) (map[string][]byte, error) {
	transport, err := a.transport(true)
	if err != nil {
		return nil, err
	}

	targets, err := a.runner.Run(ctx, transport.executable, transport.targetsArgs(), nil)
	if err != nil {
		if ctx.Err() != nil {
			return nil, ctx.Err()
		}
		if len(targets) == 0 {
			return nil, nil
		}
		return nil, fmt.Errorf("%w: could not list clipboard formats", ErrDesktopClipboardUnavailable)
	}
	available := make(map[string]string)
	for _, target := range strings.FieldsFunc(string(targets), func(r rune) bool {
		return r == '\x00' || r == '\r' || r == '\n' || r == ' ' || r == '\t'
	}) {
		available[strings.ToLower(target)] = target
	}

	formats := make(map[string][]byte)
	for _, mime := range []string{gnomeCopiedFilesMIME, uriListMIME, kdeCutSelectionMIME} {
		target, ok := available[strings.ToLower(mime)]
		if !ok {
			continue
		}
		data, err := a.runner.Run(ctx, transport.executable, transport.readArgs(target), nil)
		if err != nil {
			return nil, fmt.Errorf("%w: could not read %s", ErrDesktopClipboardUnavailable, mime)
		}
		formats[mime] = data
	}
	return formats, nil
}

func (a *linuxClipboardAdapter) Write(ctx context.Context, operation DesktopClipboardOperation, formats map[string][]byte) error {
	if a.owner != nil && a.owner.Available(ctx) {
		return a.owner.Set(ctx, formats)
	}
	if operation == DesktopClipboardCut {
		return fmt.Errorf("%w: cut needs a multi-format clipboard owner (GTK 4 introspection or equivalent)", ErrDesktopClipboardUnavailable)
	}
	data, ok := formats[uriListMIME]
	if !ok {
		return ErrInvalidOperation
	}
	transport, err := a.transport(false)
	if err != nil {
		return err
	}
	if _, err := a.runner.Run(ctx, transport.executable, transport.writeArgs(), data); err != nil {
		return fmt.Errorf("%w: could not publish file URIs", ErrDesktopClipboardUnavailable)
	}
	return nil
}

func (a *linuxClipboardAdapter) Clear(ctx context.Context) error {
	transport, err := a.transport(false)
	if err != nil {
		return err
	}
	if _, err := a.runner.Run(ctx, transport.executable, transport.clearArgs(), nil); err != nil {
		return fmt.Errorf("%w: could not clear the clipboard", ErrDesktopClipboardUnavailable)
	}
	return nil
}

type clipboardTransport struct {
	executable string
	wayland    bool
}

func (a *linuxClipboardAdapter) transport(read bool) (clipboardTransport, error) {
	wayland := a.env("WAYLAND_DISPLAY") != ""
	if wayland {
		name := "wl-copy"
		if read {
			name = "wl-paste"
		}
		if path, err := a.runner.LookPath(name); err == nil {
			return clipboardTransport{executable: path, wayland: true}, nil
		}
	}
	if a.env("DISPLAY") != "" {
		if path, err := a.runner.LookPath("xclip"); err == nil {
			return clipboardTransport{executable: path}, nil
		}
	}
	if !wayland {
		if path, err := a.runner.LookPath("wl-copy"); !read && err == nil {
			return clipboardTransport{executable: path, wayland: true}, nil
		}
		if path, err := a.runner.LookPath("wl-paste"); read && err == nil {
			return clipboardTransport{executable: path, wayland: true}, nil
		}
	}
	return clipboardTransport{}, fmt.Errorf("%w: install wl-clipboard for Wayland or xclip for X11", ErrDesktopClipboardUnavailable)
}

func (t clipboardTransport) targetsArgs() []string {
	if t.wayland {
		return []string{"--list-types"}
	}
	return []string{"-selection", "clipboard", "-o", "-target", "TARGETS"}
}

func (t clipboardTransport) readArgs(mime string) []string {
	if t.wayland {
		return []string{"--no-newline", "--type", mime}
	}
	return []string{"-selection", "clipboard", "-o", "-target", mime}
}

func (t clipboardTransport) writeArgs() []string {
	if t.wayland {
		return []string{"--type", uriListMIME}
	}
	return []string{"-selection", "clipboard", "-quiet", "-target", uriListMIME, "-i"}
}

func (t clipboardTransport) clearArgs() []string {
	if t.wayland {
		return []string{"--clear"}
	}
	return []string{"-selection", "clipboard", "-quiet", "-i"}
}

const gtkClipboardProbe = "import gi; gi.require_version('Gtk', '4.0'); from gi.repository import Gtk, Gdk, GLib"

const gtkClipboardOwnerScript = `
import base64
import json
import sys
import threading

import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gdk, GLib, Gtk

Gtk.init()
display = Gdk.Display.get_default()
if display is None:
    raise RuntimeError("No Linux desktop display is available")
clipboard = display.get_clipboard()
loop = GLib.MainLoop()

def apply(request):
    try:
        if request["command"] == "clear":
            clipboard.set_content(None)
        else:
            providers = []
            for mime, encoded in request["formats"].items():
                data = base64.b64decode(encoded, validate=True)
                providers.append(Gdk.ContentProvider.new_for_bytes(mime, GLib.Bytes.new(data)))
            provider = providers[0] if len(providers) == 1 else Gdk.ContentProvider.new_union(providers)
            if not clipboard.set_content(provider):
                raise RuntimeError("The desktop rejected the clipboard selection")
        response = {"id": request["id"]}
    except Exception as error:
        response = {"id": request.get("id", 0), "error": str(error)}
    print(json.dumps(response), flush=True)
    return GLib.SOURCE_REMOVE

def read_commands():
    for line in sys.stdin:
        try:
            request = json.loads(line)
            GLib.idle_add(apply, request)
        except Exception as error:
            print(json.dumps({"id": 0, "error": str(error)}), flush=True)
    GLib.idle_add(loop.quit)

threading.Thread(target=read_commands, daemon=True).start()
loop.run()
`

type gtkOwnerRequest struct {
	ID      uint64            `json:"id"`
	Command string            `json:"command"`
	Formats map[string]string `json:"formats,omitempty"`
}

type gtkOwnerResponse struct {
	ID    uint64 `json:"id"`
	Error string `json:"error,omitempty"`
}

type gtkOwnerLine struct {
	response gtkOwnerResponse
	err      error
}

type gtkClipboardOwner struct {
	runner clipboardCommandRunner

	probeMu sync.Mutex
	probed  bool
	python  string

	processMu sync.Mutex
	stdin     *bufio.Writer
	stdinPipe io.WriteCloser
	command   *exec.Cmd
	replies   chan gtkOwnerLine
	done      chan struct{}
	requestID uint64
}

func newGTKClipboardOwner(runner clipboardCommandRunner) *gtkClipboardOwner {
	return &gtkClipboardOwner{runner: runner}
}

func (o *gtkClipboardOwner) Available(ctx context.Context) bool {
	o.probeMu.Lock()
	defer o.probeMu.Unlock()
	if o.probed {
		return o.python != ""
	}
	python, err := o.runner.LookPath("python3")
	if err != nil {
		o.probed = true
		return false
	}
	probeContext, cancel := context.WithTimeout(ctx, 2*time.Second)
	defer cancel()
	if _, err := o.runner.Run(probeContext, python, []string{"-c", gtkClipboardProbe}, nil); err != nil {
		if ctx.Err() != nil {
			return false
		}
		o.probed = true
		return false
	}
	o.python = python
	o.probed = true
	return true
}

func (o *gtkClipboardOwner) Set(ctx context.Context, formats map[string][]byte) error {
	encoded := make(map[string]string, len(formats))
	for mime, data := range formats {
		encoded[mime] = base64.StdEncoding.EncodeToString(data)
	}
	return o.send(ctx, gtkOwnerRequest{Command: "set", Formats: encoded})
}

func (o *gtkClipboardOwner) send(ctx context.Context, request gtkOwnerRequest) error {
	o.processMu.Lock()
	defer o.processMu.Unlock()
	if err := o.start(); err != nil {
		return err
	}
	o.requestID++
	request.ID = o.requestID
	if err := json.NewEncoder(o.stdin).Encode(request); err != nil {
		o.stop()
		return fmt.Errorf("%w: could not send clipboard request", ErrDesktopClipboardUnavailable)
	}
	if err := o.stdin.Flush(); err != nil {
		o.stop()
		return fmt.Errorf("%w: could not publish clipboard request", ErrDesktopClipboardUnavailable)
	}

	for {
		select {
		case <-ctx.Done():
			o.stop()
			return ctx.Err()
		case <-o.done:
			return fmt.Errorf("%w: GTK clipboard owner exited", ErrDesktopClipboardUnavailable)
		case line := <-o.replies:
			if line.err != nil {
				o.stop()
				return fmt.Errorf("%w: GTK clipboard owner stopped", ErrDesktopClipboardUnavailable)
			}
			if line.response.ID != request.ID {
				continue
			}
			if line.response.Error != "" {
				return fmt.Errorf("%w: %s", ErrDesktopClipboardUnavailable, line.response.Error)
			}
			return nil
		}
	}
}

func (o *gtkClipboardOwner) start() error {
	if o.done != nil {
		select {
		case <-o.done:
			o.reset()
		default:
			if o.stdin != nil {
				return nil
			}
		}
	}
	o.probeMu.Lock()
	python := o.python
	o.probeMu.Unlock()
	if python == "" {
		return ErrDesktopClipboardUnavailable
	}
	command := exec.Command(python, "-u", "-c", gtkClipboardOwnerScript)
	command.Stderr = io.Discard
	stdin, err := command.StdinPipe()
	if err != nil {
		return fmt.Errorf("%w: could not start GTK clipboard owner", ErrDesktopClipboardUnavailable)
	}
	stdout, err := command.StdoutPipe()
	if err != nil {
		_ = stdin.Close()
		return fmt.Errorf("%w: could not open GTK clipboard owner output", ErrDesktopClipboardUnavailable)
	}
	if err := command.Start(); err != nil {
		_ = stdin.Close()
		_ = stdout.Close()
		return fmt.Errorf("%w: could not start GTK clipboard owner", ErrDesktopClipboardUnavailable)
	}
	o.stdin = bufio.NewWriter(stdin)
	o.stdinPipe = stdin
	o.command = command
	o.replies = make(chan gtkOwnerLine, 8)
	o.done = make(chan struct{})
	go func() {
		defer stdout.Close()
		scanner := bufio.NewScanner(stdout)
		scanner.Buffer(make([]byte, 1024), 64*1024)
		for scanner.Scan() {
			var response gtkOwnerResponse
			if err := json.Unmarshal(scanner.Bytes(), &response); err != nil {
				o.reply(gtkOwnerLine{err: err})
				continue
			}
			o.reply(gtkOwnerLine{response: response})
		}
		if err := scanner.Err(); err != nil {
			o.reply(gtkOwnerLine{err: err})
		}
		_ = command.Wait()
		close(o.done)
	}()
	return nil
}

func (o *gtkClipboardOwner) reply(line gtkOwnerLine) {
	select {
	case o.replies <- line:
	default:
	}
}

func (o *gtkClipboardOwner) stop() {
	if o.stdinPipe != nil {
		_ = o.stdinPipe.Close()
	}
	if o.command != nil && o.command.Process != nil {
		_ = o.command.Process.Kill()
	}
	if o.done != nil {
		<-o.done
	}
	o.reset()
}

func (o *gtkClipboardOwner) reset() {
	if o.stdinPipe != nil {
		_ = o.stdinPipe.Close()
	}
	o.stdin = nil
	o.stdinPipe = nil
	o.command = nil
	o.replies = nil
	o.done = nil
}
