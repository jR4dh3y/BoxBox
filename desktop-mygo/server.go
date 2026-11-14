package main

import (
	"bufio"
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"sync"
	"syscall"
	"time"
)

// desktopServer is the private BoxBox server this app runs on loopback.
type desktopServer struct {
	cmd      *exec.Cmd
	stdin    io.WriteCloser
	working  string
	origin   string
	token    string
	mounts   []mount
	ready    chan struct{} // closed when the server announced itself
	exited   chan struct{} // closed when the process exits
	stopping chan struct{} // closed when stop begins
	stopOnce sync.Once
	client   *http.Client
}

// mount is a top-level virtual folder and the local folder it shows.
type mount struct {
	Name     string `json:"name"`
	Path     string `json:"path"`
	ReadOnly bool   `json:"read_only"`
}

const desktopCookie = "boxbox_desktop"

func desktopMounts(home string) []mount {
	mounts := []mount{{Name: "root", Path: "/"}, {Name: "home", Path: home}}
	for _, name := range []string{"Desktop", "Downloads", "Documents", "Music", "Pictures", "Videos"} {
		path := filepath.Join(home, name)
		if _, err := os.Stat(path); err == nil {
			mounts = append(mounts, mount{Name: strings.ToLower(name), Path: path})
		}
	}
	return mounts
}

// serverEnvironment drops deployment overrides, which must not change the
// desktop's mounts, origins or credentials.
func serverEnvironment(token string, thumbnails []string) ([]string, error) {
	env := make([]string, 0, len(os.Environ())+2)
	for _, value := range os.Environ() {
		key, _, _ := strings.Cut(value, "=")
		if strings.HasPrefix(key, "BOXBOX_") || strings.HasPrefix(key, "FM_") || key == "CONFIG_PATH" {
			continue
		}
		env = append(env, value)
	}
	env = append(env, "BOXBOX_DESKTOP_TOKEN="+token)
	if thumbnails != nil {
		command, err := json.Marshal(thumbnails)
		if err != nil {
			return nil, err
		}
		env = append(env, "BOXBOX_DESKTOP_THUMBNAIL_COMMAND="+string(command))
	}
	return env, nil
}

// startServer runs the server on a loopback socket this process opens, so the
// origin is known at once: pages may load while the server starts, and their
// connections wait in the socket's backlog. ready closes when the server has
// announced itself; exited closes if it stops. thumbnails is the media
// thumbnail worker command, or nil.
func startServer(binary, dataDir string, thumbnails []string) (_ *desktopServer, err error) {
	if err := os.MkdirAll(dataDir, 0o700); err != nil {
		return nil, err
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return nil, err
	}
	working, err := os.MkdirTemp("", "boxbox-desktop-")
	if err != nil {
		return nil, err
	}
	defer func() {
		if err != nil {
			_ = os.RemoveAll(working)
		}
	}()
	var tokenBytes [32]byte
	_, _ = rand.Read(tokenBytes[:])
	token := hex.EncodeToString(tokenBytes[:])
	mounts := desktopMounts(home)
	config, err := json.Marshal(map[string]any{
		"host":             "127.0.0.1",
		"data_dir":         dataDir,
		"allow_root_mount": true,
		"mount_points":     mounts,
	})
	if err != nil {
		return nil, err
	}
	configPath := filepath.Join(working, "config.json")
	if err := os.WriteFile(configPath, config, 0o600); err != nil {
		return nil, err
	}
	env, err := serverEnvironment(token, thumbnails)
	if err != nil {
		return nil, err
	}
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return nil, err
	}
	origin := "http://" + listener.Addr().String()
	socket, err := listener.(*net.TCPListener).File()
	listener.Close() // the server holds the socket through its own descriptor
	if err != nil {
		return nil, err
	}
	defer socket.Close()

	cmd := exec.Command(binary, "--desktop", "--config", configPath)
	cmd.Dir = working
	cmd.Env = append(env, "BOXBOX_DESKTOP_LISTEN_FD=3")
	cmd.ExtraFiles = []*os.File{socket}
	cmd.Stderr = os.Stderr
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	// The server stops when stdin closes, including when this process dies.
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return nil, err
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, err
	}
	if err := cmd.Start(); err != nil {
		return nil, err
	}
	s := &desktopServer{
		cmd: cmd, stdin: stdin, working: working, origin: origin, token: token, mounts: mounts,
		exited: make(chan struct{}), stopping: make(chan struct{}), ready: make(chan struct{}),
		client: &http.Client{CheckRedirect: func(*http.Request, []*http.Request) error {
			return http.ErrUseLastResponse
		}},
	}
	go func() {
		_ = cmd.Wait()
		close(s.exited)
	}()
	go func() {
		line, _ := bufio.NewReaderSize(io.LimitReader(stdout, 4096), 4096).ReadString('\n')
		if announced, err := readyOrigin(line); err == nil && announced == origin {
			close(s.ready)
		} else {
			log.Printf("Unexpected BoxBox server readiness message %q", strings.TrimSpace(line))
			go s.stop()
		}
		_, _ = io.Copy(io.Discard, stdout)
	}()
	return s, nil
}

// readyOrigin accepts only a loopback HTTP origin.
func readyOrigin(line string) (string, error) {
	var ready struct {
		URL string `json:"url"`
	}
	if err := json.Unmarshal([]byte(line), &ready); err != nil {
		return "", fmt.Errorf("invalid server readiness message %q", strings.TrimSpace(line))
	}
	u, err := url.Parse(ready.URL)
	if err != nil || u.Scheme != "http" || u.Hostname() != "127.0.0.1" || u.Port() == "" ||
		(u.Path != "" && u.Path != "/") || u.RawQuery != "" || u.Fragment != "" || u.User != nil {
		return "", fmt.Errorf("the BoxBox server must listen on loopback, not %q", ready.URL)
	}
	return u.Scheme + "://" + u.Host, nil
}

// localPath resolves a virtual path such as "home/a.txt" to its local path.
func (s *desktopServer) localPath(virtual string) (string, bool) {
	name, rest, _ := strings.Cut(virtual, "/")
	for _, m := range s.mounts {
		if m.Name != name {
			continue
		}
		if slices.Contains(strings.Split(rest, "/"), "..") {
			return "", false
		}
		return filepath.Join(m.Path, rest), true
	}
	return "", false
}

// bootstrapURL exchanges the launch token for the session cookie, then opens next.
func (s *desktopServer) bootstrapURL(next string) string {
	return s.origin + "/_desktop?" + url.Values{"token": {s.token}, "next": {next}}.Encode()
}

// request calls the server's API with the desktop session cookie.
func (s *desktopServer) request(ctx context.Context, method, path string, body any) (*http.Response, error) {
	var reader io.Reader
	if body != nil {
		data, err := json.Marshal(body)
		if err != nil {
			return nil, err
		}
		reader = strings.NewReader(string(data))
	}
	req, err := http.NewRequestWithContext(ctx, method, s.origin+path, reader)
	if err != nil {
		return nil, err
	}
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	req.AddCookie(&http.Cookie{Name: desktopCookie, Value: s.token})
	return s.client.Do(req)
}

func (s *desktopServer) stop() {
	s.stopOnce.Do(func() {
		close(s.stopping)
		_ = s.stdin.Close()
		_ = syscall.Kill(-s.cmd.Process.Pid, syscall.SIGTERM)
		select {
		case <-s.exited:
		case <-time.After(5 * time.Second):
			_ = syscall.Kill(-s.cmd.Process.Pid, syscall.SIGKILL)
			<-s.exited
		}
		_ = os.RemoveAll(s.working)
	})
}
