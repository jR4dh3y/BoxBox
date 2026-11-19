//go:build linux

// Package nas runs BoxBox as several processes so each account's file access
// happens under its own NAS identity.
//
//   - The supervisor starts as root with only CAP_SETUID and CAP_SETGID. It
//     runs the API as the service identity and starts one worker per account
//     on request, choosing the identity from its own copy of the config.
//   - The API names an account; it never chooses a uid. A compromised API can
//     still ask for any mapped account. Workers prevent identity mix-ups, not
//     a compromised API.
//   - A worker serves filesystem.FS for one identity for its whole life and
//     exits when the API closes its connection.
//
// Nothing here sends signals across identities, so the supervisor needs no
// CAP_KILL. The API learns about shutdown when a pipe closes.
package nas

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"sync"
	"syscall"
	"time"

	"github.com/rs/zerolog/log"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fdconn"
)

const (
	// APIFlag and WorkerFlag select the role of a re-executed server binary.
	APIFlag    = "nas-api"
	WorkerFlag = "nas-worker"

	// Descriptors the supervisor passes to its children.
	controlFD  = 3
	shutdownFD = 4
	workerFD   = 3
)

// RunSupervisor starts the API and serves its worker requests until the API
// exits. It returns the exit code for the process.
func RunSupervisor(cfg model.NASConfig, apiArgs []string, shutdownGrace time.Duration) int {
	if os.Geteuid() != 0 {
		log.Error().Msg("NAS mode must start as root with only SETUID and SETGID capabilities; see docs/nas.md")
		return 1
	}
	executable, err := os.Executable()
	if err != nil {
		log.Error().Err(err).Msg("Find server executable")
		return 1
	}
	control, controlPeer, err := fdconn.Pair()
	if err != nil {
		log.Error().Err(err).Msg("Create control socket")
		return 1
	}
	shutdownRead, shutdownWrite, err := os.Pipe()
	if err != nil {
		log.Error().Err(err).Msg("Create shutdown pipe")
		return 1
	}

	api := exec.Command(executable, append([]string{"--" + APIFlag}, apiArgs...)...)
	api.Stdin, api.Stdout, api.Stderr = nil, os.Stdout, os.Stderr
	api.ExtraFiles = []*os.File{controlPeer, shutdownRead}
	api.SysProcAttr = &syscall.SysProcAttr{
		Credential: &syscall.Credential{Uid: cfg.ServiceUID, Gid: cfg.ServiceGID, Groups: []uint32{}},
		Pdeathsig:  syscall.SIGTERM,
	}
	if err := api.Start(); err != nil {
		log.Error().Err(err).Msg("Start API process; NAS mode needs the SETUID and SETGID capabilities")
		return 1
	}
	_ = controlPeer.Close()
	_ = shutdownRead.Close()
	log.Info().Int("pid", api.Process.Pid).Uint32("uid", cfg.ServiceUID).Msg("NAS supervisor started API")

	apiDone := make(chan int, 1)
	go func() {
		err := api.Wait()
		code := 0
		if exitErr, ok := errors.AsType[*exec.ExitError](err); ok {
			code = exitErr.ExitCode()
		} else if err != nil {
			code = 1
		}
		apiDone <- code
	}()

	s := &supervisor{cfg: cfg, executable: executable, control: control, served: make(chan struct{})}
	go s.serve()

	signals := make(chan os.Signal, 1)
	signal.Notify(signals, syscall.SIGTERM, syscall.SIGINT)
	select {
	case code := <-apiDone:
		return code
	case sig := <-signals:
		log.Info().Str("signal", sig.String()).Msg("NAS supervisor stopping API")
		_ = shutdownWrite.Close()
		select {
		case code := <-apiDone:
			s.wait()
			return code
		case <-time.After(shutdownGrace):
			log.Warn().Msg("API did not stop in time")
			return 1
		}
	}
}

type supervisor struct {
	cfg        model.NASConfig
	executable string
	control    *fdconn.Conn
	// served closes when serve returns; no worker is registered after that.
	served  chan struct{}
	workers sync.WaitGroup
}

func (s *supervisor) serve() {
	defer close(s.served)
	for {
		request, err := s.control.Receive()
		if err != nil {
			// The API exited; RunSupervisor sees that through Wait.
			return
		}
		if request.File != nil {
			_ = request.File.Close()
		}
		account := string(request.Payload)
		apiEnd, err := s.start(account)
		reply := []byte(nil)
		if err != nil {
			log.Warn().Err(err).Str("account", account).Msg("Could not start NAS worker")
			reply = []byte(err.Error())
		}
		sent := s.control.Send(request.ID, reply, apiEnd)
		if apiEnd != nil {
			// Closing our copy is enough to make an undelivered worker exit.
			_ = apiEnd.Close()
		}
		if sent != nil && !s.replyFailure(request.ID, sent) {
			return
		}
	}
}

// replyFailure tells the API a reply was lost. The API waits on every dial
// with the control lock held, so if even this fails the connection is closed:
// every dial then fails instead of blocking file access for all accounts.
func (s *supervisor) replyFailure(id uint64, err error) bool {
	log.Warn().Err(err).Msg("Reply to API")
	if s.control.Send(id, []byte("supervisor could not hand over the worker: "+err.Error()), nil) == nil {
		return true
	}
	_ = s.control.Close()
	return false
}

// start runs a worker for account and returns the API's end of its socket.
func (s *supervisor) start(account string) (*os.File, error) {
	identity, ok := s.cfg.Identities[account]
	if !ok {
		return nil, fmt.Errorf("account %q has no NAS identity", account)
	}
	apiEnd, workerEnd, err := fdconn.SocketPair()
	if err != nil {
		return nil, err
	}
	defer workerEnd.Close()
	worker := exec.Command(s.executable, "--"+WorkerFlag)
	// Workers get no environment: it holds the JWT secret and password hashes.
	worker.Env = []string{}
	worker.Dir = "/"
	worker.Stdin, worker.Stdout, worker.Stderr = nil, nil, os.Stderr
	worker.ExtraFiles = []*os.File{workerEnd}
	worker.SysProcAttr = &syscall.SysProcAttr{
		Credential: &syscall.Credential{Uid: identity.UID, Gid: identity.GID, Groups: identity.Groups},
		Pdeathsig:  syscall.SIGKILL,
	}
	if err := worker.Start(); err != nil {
		_ = apiEnd.Close()
		return nil, fmt.Errorf("start worker: %w", err)
	}
	log.Info().Str("account", account).Int("pid", worker.Process.Pid).Uint32("uid", identity.UID).Uint32("gid", identity.GID).Msg("NAS worker started")
	s.workers.Go(func() {
		err := worker.Wait()
		log.Info().Str("account", account).Int("pid", worker.Process.Pid).AnErr("exit", err).Msg("NAS worker exited")
	})
	return apiEnd, nil
}

// wait reaps workers, which exit once the API's sockets close. serve must
// finish first: a worker it is still starting would miss an earlier Wait.
func (s *supervisor) wait() {
	done := make(chan struct{})
	go func() {
		<-s.served
		s.workers.Wait()
		close(done)
	}()
	select {
	case <-done:
	case <-time.After(5 * time.Second):
	}
}
