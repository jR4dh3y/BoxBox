//go:build linux

package service

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"golang.org/x/sys/unix"
)

func TestDesktopTrashIdentityIncludesCtime(t *testing.T) {
	location := desktopTrashLocation{root: "/trash", relative: "Trash", home: true}
	first := unix.Stat_t{Dev: 7, Ino: 42, Ctim: unix.Timespec{Sec: 100, Nsec: 200}}
	second := first
	second.Ctim.Nsec++

	if trashItemID(location, "notes.txt", first) == trashItemID(location, "notes.txt", second) {
		t.Fatal("different ctime generations produced the same trash ID")
	}
}

func TestDesktopTrashMoveListRestoreAndDelete(t *testing.T) {
	root := t.TempDir()
	dataHome := filepath.Join(root, "data")
	if err := os.Mkdir(dataHome, 0700); err != nil {
		t.Fatal(err)
	}
	source := filepath.Join(root, "notes.txt")
	if err := os.WriteFile(source, []byte("daily notes"), 0600); err != nil {
		t.Fatal(err)
	}
	deletedAt := time.Date(2026, 10, 22, 12, 34, 56, 0, time.Local)
	trash := newTestDesktopTrash(t, dataHome, []model.MountPoint{{Name: "home", Path: root}}, []linuxMountInfo{{path: root, fstype: "ext4"}}, deletedAt)

	item, err := trash.Trash(context.Background(), "home/notes.txt")
	if err != nil {
		t.Fatal(err)
	}
	if item.Name != "notes.txt" || item.OriginalPath != source || !item.CanRestore || !item.DeletedAt.Equal(deletedAt) {
		t.Fatalf("unexpected trashed item: %#v", item)
	}
	if _, err := os.Lstat(source); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("source still exists after trash: %v", err)
	}

	infoPath := filepath.Join(dataHome, "Trash", "info", "notes.txt.trashinfo")
	info, err := os.ReadFile(infoPath)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(info), "[Trash Info]\nPath="+source+"\nDeletionDate=2026-10-22T12:34:56\n") {
		t.Fatalf("freedesktop metadata does not preserve source and time: %q", info)
	}

	items, err := trash.List(context.Background())
	if err != nil {
		t.Fatalf("list trashed item: %v", err)
	}
	if len(items) != 1 {
		t.Fatalf("listed %d items, want one: %#v", len(items), items)
	}
	if items[0].ID != item.ID {
		t.Fatalf("Trash returned ID %q, List returned %q after rename", item.ID, items[0].ID)
	}
	if !items[0].CanRestore {
		t.Fatalf("listed item cannot be restored: %#v", items[0])
	}
	if err := trash.Restore(context.Background(), item.ID); err != nil {
		t.Fatal(err)
	}
	restored, err := os.ReadFile(source)
	if err != nil || string(restored) != "daily notes" {
		t.Fatalf("restored content = %q, error = %v", restored, err)
	}

	deletedItem, err := trash.Trash(context.Background(), "home/notes.txt")
	if err != nil {
		t.Fatal(err)
	}
	if err := trash.Delete(context.Background(), deletedItem.ID); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Lstat(filepath.Join(dataHome, "Trash", "files", "notes.txt")); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("permanently deleted item still exists: %v", err)
	}
}

func TestDesktopTrashDefaultOrder(t *testing.T) {
	root := t.TempDir()
	dataHome := filepath.Join(root, "data")
	if err := os.Mkdir(dataHome, 0700); err != nil {
		t.Fatal(err)
	}
	stamp := time.Date(2026, 1, 1, 0, 0, 0, 0, time.Local)
	trash := newTestDesktopTrash(t, dataHome, []model.MountPoint{{Name: "home", Path: root}}, []linuxMountInfo{{path: root, fstype: "ext4"}}, stamp)
	for _, name := range []string{"old-folder", "new-folder", "file10.txt", "file2.txt", "newest.txt"} {
		if strings.HasSuffix(name, "folder") {
			if err := os.Mkdir(filepath.Join(root, name), 0700); err != nil {
				t.Fatal(err)
			}
		} else if err := os.WriteFile(filepath.Join(root, name), []byte("test"), 0600); err != nil {
			t.Fatal(err)
		}
		modified := stamp
		if name == "new-folder" || name == "newest.txt" {
			modified = stamp.Add(time.Hour)
		}
		if err := os.Chtimes(filepath.Join(root, name), modified, modified); err != nil {
			t.Fatal(err)
		}
		if _, err := trash.Trash(context.Background(), "home/"+name); err != nil {
			t.Fatal(err)
		}
	}
	items, err := trash.List(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	names := make([]string, len(items))
	for i, item := range items {
		names[i] = item.Name
	}
	want := []string{"new-folder", "old-folder", "newest.txt", "file2.txt", "file10.txt"}
	if !reflect.DeepEqual(names, want) {
		t.Fatalf("order = %v, want %v", names, want)
	}
}

func TestDesktopTrashStaleIDCannotRestoreOrDeleteReplacement(t *testing.T) {
	root := t.TempDir()
	dataHome := filepath.Join(root, "data")
	if err := os.Mkdir(dataHome, 0700); err != nil {
		t.Fatal(err)
	}
	source := filepath.Join(root, "notes.txt")
	if err := os.WriteFile(source, []byte("original"), 0600); err != nil {
		t.Fatal(err)
	}
	trash := newTestDesktopTrash(t, dataHome, []model.MountPoint{{Name: "home", Path: root}}, []linuxMountInfo{{path: root, fstype: "ext4"}}, time.Now())

	stale, err := trash.Trash(context.Background(), "home/notes.txt")
	if err != nil {
		t.Fatal(err)
	}
	trashFile := filepath.Join(dataHome, "Trash", "files", "notes.txt")
	trashInfo := filepath.Join(dataHome, "Trash", "info", "notes.txt.trashinfo")
	if err := os.Remove(trashFile); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(trashInfo); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(source, []byte("replacement"), 0600); err != nil {
		t.Fatal(err)
	}
	replacement, err := trash.Trash(context.Background(), "home/notes.txt")
	if err != nil {
		t.Fatal(err)
	}
	if replacement.ID == stale.ID {
		t.Fatalf("replacement reused stale trash ID %q", stale.ID)
	}

	for name, action := range map[string]func(context.Context, string) error{
		"restore": trash.Restore,
		"delete":  trash.Delete,
	} {
		if err := action(context.Background(), stale.ID); !errors.Is(err, ErrTrashItemNotFound) {
			t.Errorf("stale %s error = %v, want ErrTrashItemNotFound", name, err)
		}
	}
	content, err := os.ReadFile(trashFile)
	if err != nil || string(content) != "replacement" {
		t.Fatalf("replacement was changed by stale ID: content=%q err=%v", content, err)
	}
	if _, err := os.Lstat(source); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("replacement was restored by stale ID: %v", err)
	}
}

func TestDesktopTrashMovesAndRestoresTheSymlinkEntry(t *testing.T) {
	root := t.TempDir()
	dataHome := filepath.Join(root, "data")
	if err := os.Mkdir(dataHome, 0700); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(root, "target.txt")
	link := filepath.Join(root, "shortcut.txt")
	if err := os.WriteFile(target, []byte("keep the target"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("target.txt", link); err != nil {
		t.Fatal(err)
	}
	trash := newTestDesktopTrash(t, dataHome, []model.MountPoint{{Name: "home", Path: root}}, []linuxMountInfo{{path: root, fstype: "ext4"}}, time.Now())

	item, err := trash.Trash(context.Background(), "home/shortcut.txt")
	if err != nil {
		t.Fatal(err)
	}
	if info, err := os.Lstat(target); err != nil || info.Mode()&os.ModeSymlink != 0 {
		t.Fatalf("target changed during link trash: info=%v err=%v", info, err)
	}
	if content, err := os.ReadFile(target); err != nil || string(content) != "keep the target" {
		t.Fatalf("target content changed during link trash: %q err=%v", content, err)
	}
	if err := trash.Restore(context.Background(), item.ID); err != nil {
		t.Fatal(err)
	}
	info, err := os.Lstat(link)
	if err != nil || info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("restored entry is not a symlink: info=%v err=%v", info, err)
	}
	linkTarget, err := os.Readlink(link)
	if err != nil || linkTarget != "target.txt" {
		t.Fatalf("restored link target = %q, error = %v", linkTarget, err)
	}
	if content, err := os.ReadFile(target); err != nil || string(content) != "keep the target" {
		t.Fatalf("restored link changed referent: %q err=%v", content, err)
	}
}

func TestDesktopTrashRejectsMountRootsTraversalAndEscapingSymlinks(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	dataHome := filepath.Join(root, "data")
	if err := os.Mkdir(dataHome, 0700); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(outside, "secret.txt")
	if err := os.WriteFile(target, []byte("untouched"), 0600); err != nil {
		t.Fatal(err)
	}
	link := filepath.Join(root, "outside-link")
	if err := os.Symlink(target, link); err != nil {
		t.Fatal(err)
	}
	parentLink := filepath.Join(root, "outside-directory")
	if err := os.Symlink(outside, parentLink); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(outside, "nested.txt"), []byte("also untouched"), 0600); err != nil {
		t.Fatal(err)
	}
	trash := newTestDesktopTrash(t, dataHome, []model.MountPoint{{Name: "home", Path: root}}, []linuxMountInfo{{path: root, fstype: "ext4"}}, time.Now())

	for _, path := range []string{"home", "home/../outside-link", "home/outside-link", "home/outside-directory/nested.txt"} {
		if _, err := trash.Trash(context.Background(), path); err == nil {
			t.Errorf("Trash(%q) unexpectedly succeeded", path)
		}
	}
	if info, err := os.Lstat(link); err != nil || info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("escaping symlink entry changed: info=%v err=%v", info, err)
	}
	if content, err := os.ReadFile(target); err != nil || string(content) != "untouched" {
		t.Fatalf("escaping symlink referent changed: %q err=%v", content, err)
	}
	if content, err := os.ReadFile(filepath.Join(outside, "nested.txt")); err != nil || string(content) != "also untouched" {
		t.Fatalf("path through escaping parent changed: %q err=%v", content, err)
	}
}

func TestDesktopTrashCollisionAndRestoreNeverOverwrite(t *testing.T) {
	root := t.TempDir()
	dataHome := filepath.Join(root, "data")
	if err := os.Mkdir(dataHome, 0700); err != nil {
		t.Fatal(err)
	}
	source := filepath.Join(root, "same.txt")
	if err := os.WriteFile(source, []byte("first"), 0600); err != nil {
		t.Fatal(err)
	}
	trash := newTestDesktopTrash(t, dataHome, []model.MountPoint{{Name: "home", Path: root}}, []linuxMountInfo{{path: root, fstype: "ext4"}}, time.Now())
	first, err := trash.Trash(context.Background(), "home/same.txt")
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(source, []byte("second"), 0600); err != nil {
		t.Fatal(err)
	}
	second, err := trash.Trash(context.Background(), "home/same.txt")
	if err != nil {
		t.Fatal(err)
	}

	entries, err := os.ReadDir(filepath.Join(dataHome, "Trash", "files"))
	if err != nil {
		t.Fatal(err)
	}
	names := make([]string, len(entries))
	for i, entry := range entries {
		names[i] = entry.Name()
	}
	sort.Strings(names)
	if fmt.Sprint(names) != "[same.txt same.txt.1]" {
		t.Fatalf("collision names = %v", names)
	}
	if err := trash.Restore(context.Background(), first.ID); err != nil {
		t.Fatal(err)
	}
	if err := trash.Restore(context.Background(), second.ID); !errors.Is(err, ErrTrashConflict) {
		t.Fatalf("restore over existing path error = %v", err)
	}
	content, err := os.ReadFile(source)
	if err != nil || string(content) != "first" {
		t.Fatalf("restore overwrote the original entry: %q err=%v", content, err)
	}
	if err := trash.Delete(context.Background(), second.ID); err != nil {
		t.Fatal(err)
	}
}

func TestDesktopTrashUsesLocalVolumeTrashAndRejectsUnavailableVolumeTrash(t *testing.T) {
	t.Run("sticky shared trash directory", func(t *testing.T) {
		volume := t.TempDir()
		home := t.TempDir()
		dataHome := filepath.Join(home, "data")
		if err := os.Mkdir(dataHome, 0700); err != nil {
			t.Fatal(err)
		}
		shared := filepath.Join(volume, ".Trash")
		if err := os.Mkdir(shared, 0700); err != nil {
			t.Fatal(err)
		}
		if err := os.Chmod(shared, os.ModeSticky|0777); err != nil {
			t.Fatal(err)
		}
		source := filepath.Join(volume, "photo.jpg")
		if err := os.WriteFile(source, []byte("photo"), 0600); err != nil {
			t.Fatal(err)
		}
		trash := newTestDesktopTrash(t, dataHome, []model.MountPoint{{Name: "drive", Path: volume}}, []linuxMountInfo{{path: volume, fstype: "ext4"}, {path: home, fstype: "ext4"}}, time.Now())
		item, err := trash.Trash(context.Background(), "drive/photo.jpg")
		if err != nil {
			t.Fatal(err)
		}
		volumeTrash := filepath.Join(shared, strconv.Itoa(os.Getuid()))
		if _, err := os.Stat(filepath.Join(volumeTrash, "files", "photo.jpg")); err != nil {
			t.Fatalf("same-volume item missing from shared Trash: %v", err)
		}
		if items, err := trash.List(context.Background()); err != nil || len(items) != 1 || items[0].ID != item.ID {
			t.Fatalf("volume trash list = %#v, error = %v", items, err)
		}
	})

	t.Run("unavailable local trash", func(t *testing.T) {
		volume := t.TempDir()
		home := t.TempDir()
		dataHome := filepath.Join(home, "data")
		if err := os.Mkdir(dataHome, 0700); err != nil {
			t.Fatal(err)
		}
		source := filepath.Join(volume, "photo.jpg")
		if err := os.WriteFile(source, []byte("photo"), 0600); err != nil {
			t.Fatal(err)
		}
		outside := t.TempDir()
		if err := os.Symlink(outside, filepath.Join(volume, ".Trash-"+strconv.Itoa(os.Getuid()))); err != nil {
			t.Fatal(err)
		}
		trash := newTestDesktopTrash(t, dataHome, []model.MountPoint{{Name: "drive", Path: volume}}, []linuxMountInfo{{path: volume, fstype: "ext4"}, {path: home, fstype: "ext4"}}, time.Now())
		if _, err := trash.Trash(context.Background(), "drive/photo.jpg"); !errors.Is(err, ErrTrashUnavailable) {
			t.Fatalf("Trash on a volume without a safe local Trash returned %v", err)
		}
		if content, err := os.ReadFile(source); err != nil || string(content) != "photo" {
			t.Fatalf("failed Trash changed source: %q err=%v", content, err)
		}
		entries, err := os.ReadDir(outside)
		if err != nil || len(entries) != 0 {
			t.Fatalf("symlink target was modified: entries=%v err=%v", entries, err)
		}
	})

	t.Run("remote filesystem is refused", func(t *testing.T) {
		volume := t.TempDir()
		home := t.TempDir()
		dataHome := filepath.Join(home, "data")
		if err := os.Mkdir(dataHome, 0700); err != nil {
			t.Fatal(err)
		}
		source := filepath.Join(volume, "photo.jpg")
		if err := os.WriteFile(source, []byte("photo"), 0600); err != nil {
			t.Fatal(err)
		}
		trash := newTestDesktopTrash(t, dataHome, []model.MountPoint{{Name: "drive", Path: volume}}, []linuxMountInfo{{path: volume, fstype: "nfs4"}, {path: home, fstype: "ext4"}}, time.Now())
		if _, err := trash.Trash(context.Background(), "drive/photo.jpg"); !errors.Is(err, ErrTrashUnavailable) {
			t.Fatalf("Trash on a remote filesystem returned %v", err)
		}
		if content, err := os.ReadFile(source); err != nil || string(content) != "photo" {
			t.Fatalf("refused remote Trash changed source: %q err=%v", content, err)
		}
	})
}

func newTestDesktopTrash(t *testing.T, dataHome string, mountPoints []model.MountPoint, mounts []linuxMountInfo, now time.Time) *desktopTrashService {
	t.Helper()
	mountInfoPath := filepath.Join(t.TempDir(), "mountinfo")
	var contents strings.Builder
	for index, mount := range mounts {
		fmt.Fprintf(&contents, "%d 1 0:1 / %s rw - %s /dev/test rw\n", index+2, escapeMountInfoPath(mount.path), mount.fstype)
	}
	if err := os.WriteFile(mountInfoPath, []byte(contents.String()), 0600); err != nil {
		t.Fatal(err)
	}
	return newDesktopTrashService(filesystem.NewOsFS(), mountPoints, desktopTrashConfig{
		dataHome:      dataHome,
		mountInfoPath: mountInfoPath,
		now:           func() time.Time { return now },
	})
}

func escapeMountInfoPath(path string) string {
	path = strings.ReplaceAll(path, "\\", "\\134")
	path = strings.ReplaceAll(path, " ", "\\040")
	path = strings.ReplaceAll(path, "\t", "\\011")
	return strings.ReplaceAll(path, "\n", "\\012")
}
