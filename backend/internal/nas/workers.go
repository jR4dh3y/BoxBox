//go:build linux

package nas

import (
	"errors"
	"fmt"
	"io"
	"os"
	"sync"
	"syscall"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fdconn"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/workerfs"
)

// workerIdle is how long an account's worker lives without file calls.
const workerIdle = 5 * time.Minute

// Workers is the API's side of the supervisor: one filesystem per mapped
// account, each backed by a worker running as that account's identity.
type Workers struct {
	mu      sync.Mutex
	control *fdconn.Conn
	nextID  uint64
	fs      map[string]*workerfs.Client
}

// ConnectToSupervisor uses the descriptors the supervisor passed to the API.
func ConnectToSupervisor(cfg model.NASConfig) (*Workers, error) {
	if os.Geteuid() == 0 {
		return nil, errors.New("the NAS API process must not run as root")
	}
	control, err := fdconn.New(os.NewFile(controlFD, "supervisor"))
	if err != nil {
		return nil, fmt.Errorf("connect to NAS supervisor: %w", err)
	}
	workers := &Workers{control: control, fs: make(map[string]*workerfs.Client)}
	for account := range cfg.Identities {
		workers.fs[account] = workerfs.NewClient(func() (*fdconn.Conn, error) {
			return workers.dial(account)
		}, workerIdle)
	}
	return workers, nil
}

// FS returns the account's filesystem, or false for an unmapped account.
func (w *Workers) FS(account string) (filesystem.FS, bool) {
	fs, ok := w.fs[account]
	return fs, ok
}

// Accounts lists every mapped account.
func (w *Workers) Accounts() []string {
	accounts := make([]string, 0, len(w.fs))
	for account := range w.fs {
		accounts = append(accounts, account)
	}
	return accounts
}

func (w *Workers) dial(account string) (*fdconn.Conn, error) {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.nextID++
	if err := w.control.Send(w.nextID, []byte(account), nil); err != nil {
		return nil, fmt.Errorf("ask supervisor for a worker: %w", err)
	}
	reply, err := w.control.Receive()
	if err != nil {
		return nil, fmt.Errorf("supervisor did not reply: %w", err)
	}
	if reply.ID != w.nextID {
		if reply.File != nil {
			_ = reply.File.Close()
		}
		return nil, errors.New("supervisor replied out of order")
	}
	if len(reply.Payload) > 0 {
		if reply.File != nil {
			_ = reply.File.Close()
		}
		return nil, errors.New(string(reply.Payload))
	}
	if reply.File == nil {
		return nil, errors.New("supervisor sent no worker socket")
	}
	return fdconn.New(reply.File)
}

// SupervisorConfig is the configuration the supervisor loaded, readable once.
// The API reads it instead of config.yaml, which may be readable only by root.
func SupervisorConfig() io.ReadCloser {
	return os.NewFile(configFD, "supervisor-config")
}

// WatchShutdown sends SIGTERM on signals once the supervisor asks the API to
// exit, so shutdown takes the same path as a real signal would.
func WatchShutdown(signals chan<- os.Signal) {
	pipe := os.NewFile(shutdownFD, "shutdown")
	go func() {
		_, _ = io.Copy(io.Discard, pipe)
		signals <- syscall.SIGTERM
	}()
}
