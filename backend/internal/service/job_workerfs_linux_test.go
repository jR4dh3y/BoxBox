package service

import (
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fdconn"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/workerfs"
)

// NAS mode copies through a file worker, which must create the links and set
// the modes itself.
func TestFolderCopyThroughWorkerKeepsSymlinksAndFileModes(t *testing.T) {
	client := workerfs.NewClient(func() (*fdconn.Conn, error) {
		local, peer, err := fdconn.Pair()
		if err != nil {
			return nil, err
		}
		remote, err := fdconn.New(peer)
		if err != nil {
			return nil, err
		}
		go func() { _ = workerfs.Serve(remote, filesystem.NewOsFS()) }()
		return local, nil
	}, 0)
	t.Cleanup(func() { _ = client.Close() })
	requireFolderCopyKeepsLinksAndModes(t, client)
}
