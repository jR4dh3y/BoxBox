//go:build linux

package nas

import (
	"errors"
	"fmt"
	"os"

	"golang.org/x/sys/unix"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fdconn"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/workerfs"
)

// RunWorker serves the real filesystem to the API on the inherited socket
// until the API closes it.
func RunWorker() error {
	if err := unix.Prctl(unix.PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0); err != nil {
		return fmt.Errorf("set no_new_privs: %w", err)
	}
	if os.Geteuid() == 0 || os.Getegid() == 0 {
		return errors.New("a NAS worker must not run as root")
	}
	conn, err := fdconn.New(os.NewFile(workerFD, "api"))
	if err != nil {
		return err
	}
	defer conn.Close()
	return workerfs.Serve(conn, filesystem.NewOsFS())
}
