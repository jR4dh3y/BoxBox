package service

import (
	"context"
	"errors"
	"io"
	"strings"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

// Each account sees its own filesystem here, as with per-account NAS workers.
func TestShareRecipientsUseTheCreatorsFiles(t *testing.T) {
	store := filesystem.NewMemMapFS()
	_ = store.MkdirAll("/data", 0o755)
	alice := filesystem.NewMemMapFS()
	_ = alice.MkdirAll("/data/media/upload", 0o755)
	_ = alice.WriteFile("/data/media/note.txt", []byte("alice's note"), 0o644)
	bob := filesystem.NewMemMapFS()
	_ = bob.MkdirAll("/data/media", 0o755)
	mapped := map[string]filesystem.FS{"alice": alice, "bob": bob}
	shares := NewShareService(store, ShareServiceConfig{
		DataDir: "/data",
		Mounts:  func() []model.MountPoint { return []model.MountPoint{{Name: "media", Path: "/data/media"}} },
		Files: func(account string) (filesystem.FS, error) {
			if fsys, ok := mapped[account]; ok {
				return fsys, nil
			}
			return nil, ErrNoFileAccess
		},
	})
	ctx := context.Background()

	// Bob cannot share a file only Alice can reach.
	if _, err := shares.Create(ctx, "bob", "media/note.txt", ShareSettings{}, time.Time{}); !errors.Is(err, ErrPathNotFound) {
		t.Fatalf("bob shared alice's file: %v", err)
	}
	if _, err := shares.Create(ctx, "carol", "media/note.txt", ShareSettings{}, time.Time{}); !errors.Is(err, ErrNoFileAccess) {
		t.Fatalf("unmapped account created a share: %v", err)
	}

	link, err := shares.Create(ctx, "alice", "media/note.txt", ShareSettings{}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	file, _, err := shares.OpenForRecipient(ctx, link.Token)
	if err != nil {
		t.Fatal(err)
	}
	content, _ := io.ReadAll(file)
	_ = file.Close()
	if string(content) != "alice's note" {
		t.Fatalf("recipient read %q", content)
	}

	folder, err := shares.Create(ctx, "alice", "media/upload", ShareSettings{Permissions: model.SharePermissions{Upload: true}}, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if _, _, err := shares.WriteForRecipientPath(ctx, folder.Token, "from-visitor.txt", strings.NewReader("hi")); err != nil {
		t.Fatal(err)
	}
	if exists, _ := alice.Exists("/data/media/upload/from-visitor.txt"); !exists {
		t.Fatal("upload did not land in the creator's filesystem")
	}
	if exists, _ := store.Exists("/data/media/upload/from-visitor.txt"); exists {
		t.Fatal("upload landed in the API's own filesystem")
	}

	// A creator who loses file access takes their links with them.
	delete(mapped, "alice")
	if _, _, err := shares.OpenForRecipient(ctx, link.Token); !errors.Is(err, ErrShareNotFound) {
		t.Fatalf("link outlived its creator's access: %v", err)
	}
}

// Expiry cleanup cannot delete as a share's creator, so with per-account files
// a delete-on-expiry link is refused and the cleanup never starts.
func TestShareDeleteOnExpiryIsOffWithPerAccountFiles(t *testing.T) {
	store := filesystem.NewMemMapFS()
	_ = store.MkdirAll("/data", 0o755)
	alice := filesystem.NewMemMapFS()
	_ = alice.WriteFile("/data/media/note.txt", []byte("alice's note"), 0o644)
	shares := NewShareService(store, ShareServiceConfig{
		DataDir: "/data",
		Mounts:  func() []model.MountPoint { return []model.MountPoint{{Name: "media", Path: "/data/media"}} },
		Files:   func(string) (filesystem.FS, error) { return alice, nil },
	}).(*shareService)
	ctx := context.Background()
	expiresAt := time.Now().Add(time.Hour)

	if _, err := shares.Create(ctx, "alice", "media/note.txt", ShareSettings{DeleteOnExpiry: true}, expiresAt); !errors.Is(err, ErrInvalidOperation) {
		t.Fatalf("Create with delete on expiry = %v, want ErrInvalidOperation", err)
	}
	if _, err := shares.Create(ctx, "alice", "media/note.txt", ShareSettings{}, expiresAt); err != nil {
		t.Fatalf("Create without delete on expiry = %v", err)
	}

	cleanupCtx, cancel := context.WithCancel(ctx)
	defer cancel()
	shares.StartExpiryCleanup(cleanupCtx)
	// Nothing started, so stopping returns well before the deadline.
	stopCtx, stop := context.WithTimeout(ctx, time.Second)
	defer stop()
	if err := shares.StopExpiryCleanup(stopCtx); err != nil {
		t.Fatalf("StopExpiryCleanup = %v, want nil", err)
	}
}
