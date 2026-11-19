//go:build !linux

package nas

import (
	"errors"
	"time"

	"github.com/rs/zerolog/log"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

const (
	APIFlag    = "nas-api"
	WorkerFlag = "nas-worker"
)

var errUnsupported = errors.New("NAS mode requires Linux")

type Workers struct{}

func RunSupervisor(model.NASConfig, []string, time.Duration) int {
	log.Error().Err(errUnsupported).Msg("Cannot start NAS mode")
	return 1
}

func RunWorker() error { return errUnsupported }

func ConnectToSupervisor(model.NASConfig) (*Workers, error) { return nil, errUnsupported }

func (*Workers) FS(string) (filesystem.FS, bool) { return nil, false }

func (*Workers) Accounts() []string { return nil }

func WatchShutdown() {}
