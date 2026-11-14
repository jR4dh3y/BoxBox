// Command boxbox-mygo is the BoxBox desktop app on the system WebKitGTK.
//
// It runs the private BoxBox server on loopback and shows its Svelte frontend
// in MyGo windows; see README.md.
package main

import (
	"context"
	"errors"
	"fmt"
	"log"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"
	"time"

	"github.com/egoist/mygo"
)

func main() {
	if err := run(); err != nil {
		log.Printf("BoxBox: %v", err)
		os.Exit(1)
	}
}

func run() error {
	cwd, err := os.Getwd()
	if err != nil {
		return err
	}
	opts, err := parseLaunch(os.Args[1:], cwd, os.Getenv("BOXBOX_DESKTOP_URL"))
	if err != nil {
		return err
	}
	exe, err := os.Executable()
	if err != nil {
		return err
	}
	if opts.chooser != "" {
		// An AppImage runs from a temporary mount; the image itself persists.
		launcher := exe
		if image := os.Getenv("APPIMAGE"); filepath.IsAbs(image) {
			launcher = image
		}
		path, err := installChooserPortal(launcher, opts.chooser == "enable", os.Getenv)
		if err != nil {
			return err
		}
		fmt.Printf("Updated FileChooser routing in %s. Log in again to apply.\n", path)
		return nil
	}

	mygo.App.SetName("BoxBox")
	// A later launch hands its command line to this one and exits; the
	// remote URL in its environment travels as an argument.
	if remote := os.Getenv("BOXBOX_DESKTOP_URL"); remote != "" {
		os.Args = append(os.Args, "--remote", remote)
	}
	if !mygo.App.RequestSingleInstanceLock() {
		return nil
	}

	dataDir := opts.dataDir
	if dataDir == "" {
		config, err := os.UserConfigDir()
		if err != nil {
			return err
		}
		dataDir = filepath.Join(config, "boxbox")
	}
	server, err := startServer(serverBinary(exe, opts.server), dataDir, thumbnailCommand(exe))
	if err != nil {
		return err
	}
	defer server.stop()

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	drag := &gtkDrag{}
	mygo.Bind(&DesktopDrag{server: server, drag: drag})
	accelerated := hardwareAcceleration(dataDir)
	mygo.App.SetDMABUFRenderer(renderer(accelerated))
	app := newDesktopApp(ctx, server, drag, accelerated)

	mygo.App.OnBeforeQuit(func(*mygo.QuitEvent) {
		cancel()
		server.stop()
	})
	mygo.App.OnSecondInstance(func(args []string, workingDir string) {
		opts, err := parseLaunch(args, workingDir, "")
		if err != nil {
			log.Printf("Ignored a launch: %v", err)
			return
		}
		app.handle(opts.request)
	})
	mygo.App.WhenReady(func() {
		app.handle(opts.request)
		go app.watchChoosers()
	})
	go func() {
		select {
		case <-server.ready:
		case <-server.exited:
		case <-time.After(30 * time.Second):
			log.Print("The local BoxBox server did not start in time")
			mygo.App.Quit()
			return
		}
		select {
		case <-server.exited:
			log.Print("The local BoxBox server exited")
			mygo.App.Quit()
		case <-server.stopping:
		}
	}()
	stopSignal := make(chan os.Signal, 1)
	signal.Notify(stopSignal, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		<-stopSignal
		mygo.App.Quit()
	}()
	if err := mygo.App.Run(); err != nil && !errors.Is(err, syscall.EINTR) {
		return err
	}
	return nil
}

// serverBinary is the --server flag, the server beside this executable in a
// bundle, or the repository's build for development.
func serverBinary(exe, flag string) string {
	if flag != "" {
		return flag
	}
	if bundled := filepath.Join(filepath.Dir(exe), "boxbox-server"); isFile(bundled) {
		return bundled
	}
	return filepath.Join(filepath.Dir(exe), "../dist/boxbox")
}

// thumbnailCommand is the media thumbnail worker beside this executable, or
// nil, which leaves the server's built-in image thumbnails.
func thumbnailCommand(exe string) []string {
	dir := filepath.Dir(exe)
	worker, library := filepath.Join(dir, "boxbox-thumbnail"), filepath.Join(dir, "libboxbox_thumbnail.so")
	if !isFile(worker) || !isFile(library) {
		return nil
	}
	return []string{worker, library}
}

func isFile(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.Mode().IsRegular()
}
