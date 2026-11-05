// Command boxbox-mygo is a no-CEF desktop host prototype for BoxBox.
//
// It deliberately keeps the existing authenticated local HTTP server and
// Svelte frontend. MyGo supplies only the native window and WebKitGTK webview.
package main

import (
	"bufio"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"log"
	"net"
	"net/url"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/egoist/mygo"
)

type desktopServer struct {
	cmd       *exec.Cmd
	stdin     io.WriteCloser
	working   string
	bootstrap string
	stopOnce  sync.Once
}

func newDesktopServer(binary, dataDir string) (*desktopServer, error) {
	working, err := os.MkdirTemp("", "boxbox-mygo-")
	if err != nil {
		return nil, err
	}
	if err := os.MkdirAll(dataDir, 0o700); err != nil {
		_ = os.RemoveAll(working)
		return nil, err
	}

	var tokenBytes [32]byte
	if _, err := rand.Read(tokenBytes[:]); err != nil {
		_ = os.RemoveAll(working)
		return nil, err
	}
	token := hex.EncodeToString(tokenBytes[:])
	home, err := os.UserHomeDir()
	if err != nil {
		_ = os.RemoveAll(working)
		return nil, err
	}

	mounts := []map[string]any{{"name": "root", "path": "/", "read_only": false}, {"name": "home", "path": home, "read_only": false}}
	for _, name := range []string{"Desktop", "Downloads", "Documents", "Music", "Pictures", "Videos"} {
		path := filepath.Join(home, name)
		if _, err := os.Stat(path); err == nil {
			mounts = append(mounts, map[string]any{"name": strings.ToLower(name), "path": path, "read_only": false})
		}
	}
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		_ = os.RemoveAll(working)
		return nil, err
	}
	port := listener.Addr().(*net.TCPAddr).Port
	_ = listener.Close()

	config := map[string]any{
		"port":             port,
		"host":             "127.0.0.1",
		"data_dir":         dataDir,
		"allow_root_mount": true,
		"mount_points":     mounts,
	}
	configBytes, err := json.Marshal(config)
	if err != nil {
		_ = os.RemoveAll(working)
		return nil, err
	}
	configPath := filepath.Join(working, "config.json")
	if err := os.WriteFile(configPath, configBytes, 0o600); err != nil {
		_ = os.RemoveAll(working)
		return nil, err
	}

	env := make([]string, 0, len(os.Environ())+1)
	for _, value := range os.Environ() {
		key := strings.SplitN(value, "=", 2)[0]
		if strings.HasPrefix(key, "BOXBOX_") || strings.HasPrefix(key, "FM_") || key == "CONFIG_PATH" {
			continue
		}
		env = append(env, value)
	}
	env = append(env, "BOXBOX_DESKTOP_TOKEN="+token)

	cmd := exec.Command(binary, "--desktop", "--config", configPath)
	cmd.Dir = working
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	cmd.Env = env
	cmd.Stderr = os.Stderr
	stdin, err := cmd.StdinPipe()
	if err != nil {
		_ = os.RemoveAll(working)
		return nil, err
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		_ = os.RemoveAll(working)
		return nil, err
	}
	if err := cmd.Start(); err != nil {
		_ = os.RemoveAll(working)
		return nil, err
	}

	line, err := bufio.NewReader(stdout).ReadString('\n')
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		_ = os.RemoveAll(working)
		return nil, fmt.Errorf("server exited before readiness: %w", err)
	}
	var ready struct {
		URL string `json:"url"`
	}
	if err := json.Unmarshal([]byte(line), &ready); err != nil || ready.URL == "" {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		_ = os.RemoveAll(working)
		return nil, fmt.Errorf("invalid server readiness message %q", strings.TrimSpace(line))
	}

	bootstrap := ready.URL + "/_desktop?" + url.Values{"token": {token}, "next": {"/browse"}}.Encode()
	readyURL, err := url.Parse(ready.URL)
	if err != nil {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		_ = os.RemoveAll(working)
		return nil, fmt.Errorf("invalid server URL %q: %w", ready.URL, err)
	}
	deadline := time.Now().Add(5 * time.Second)
	for {
		connection, dialErr := net.DialTimeout("tcp", readyURL.Host, 100*time.Millisecond)
		if dialErr == nil {
			_ = connection.Close()
			break
		}
		if time.Now().After(deadline) {
			_ = cmd.Process.Kill()
			_ = cmd.Wait()
			_ = os.RemoveAll(working)
			return nil, fmt.Errorf("desktop server did not accept connections: %w", dialErr)
		}
		time.Sleep(10 * time.Millisecond)
	}
	return &desktopServer{cmd: cmd, stdin: stdin, working: working, bootstrap: bootstrap}, nil
}

func (s *desktopServer) stop() {
	if s == nil {
		return
	}
	s.stopOnce.Do(func() {
		if s.cmd == nil || s.cmd.Process == nil {
			return
		}
		if s.cmd.ProcessState == nil {
			_ = syscall.Kill(-s.cmd.Process.Pid, syscall.SIGTERM)
		}
		_ = s.stdin.Close()
		_ = s.cmd.Wait()
		_ = os.RemoveAll(s.working)
	})
}

func main() {
	serverBinary := flag.String("server", "../dist/boxbox", "path to the built BoxBox server")
	dataDir := flag.String("data-dir", "", "desktop data directory")
	flag.Parse()

	if *dataDir == "" {
		configDir, err := os.UserConfigDir()
		if err != nil {
			log.Fatal(err)
		}
		*dataDir = filepath.Join(configDir, "boxbox")
	}
	binary, err := filepath.Abs(*serverBinary)
	if err != nil {
		log.Fatal(err)
	}
	if _, err := os.Stat(binary); err != nil {
		log.Fatalf("server binary %s: %v; build it with 'bun run build:single'", binary, err)
	}

	server, err := newDesktopServer(binary, *dataDir)
	if err != nil {
		log.Fatal(err)
	}
	defer server.stop()
	log.Printf("BoxBox MyGo server: %s", server.bootstrap)

	mygo.App.SetName("BoxBox")
	mygo.App.OnBeforeQuit(func(_ *mygo.QuitEvent) {
		log.Print("BoxBox MyGo quitting")
		server.stop()
	})
	mygo.App.WhenReady(func() {
		win := mygo.NewWindow(mygo.WindowOptions{
			Title:           "BoxBox",
			URL:             server.bootstrap,
			Width:           1280,
			Height:          800,
			MinWidth:        640,
			MinHeight:       420,
			StateKey:        "main",
			BackgroundColor: "light-dark(#ffffff, #181818)",
		})
		win.Page().OnDidFinishLoad(func() { log.Printf("BoxBox WebKit page loaded: %s", win.Page().URL()) })
		win.Page().OnDidFailLoad(func(err *mygo.LoadError) {
			log.Printf("BoxBox WebKit page failed: %v", err)
		})
		win.OnClosed(func() { log.Print("BoxBox MyGo window closed") })
	})

	stopSignal := make(chan os.Signal, 1)
	signal.Notify(stopSignal, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-stopSignal
		mygo.App.Quit()
	}()
	if err := mygo.App.Run(); err != nil && !errors.Is(err, syscall.EINTR) {
		log.Fatal(err)
	}
	log.Print("BoxBox MyGo event loop ended")
}
