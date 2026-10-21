//go:build linux

package service

import (
	"bufio"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"golang.org/x/sys/unix"
)

type desktopTrashLocation struct {
	root       string
	relative   string
	originBase string
	home       bool
	volumeRoot string
}

type linuxMountInfo struct {
	path   string
	fstype string
}

func (s *desktopTrashService) newVolumeTrashLocation(volumeRoot string) (desktopTrashLocation, error) {
	rootFD, err := openAbsoluteDir(volumeRoot, false)
	if err != nil {
		return desktopTrashLocation{}, fmt.Errorf("%w: volume is unavailable", ErrTrashUnavailable)
	}
	defer unix.Close(rootFD)
	uid := strconv.Itoa(os.Getuid())

	var sharedStat unix.Stat_t
	if err := unix.Fstatat(rootFD, ".Trash", &sharedStat, unix.AT_SYMLINK_NOFOLLOW); err == nil &&
		sharedStat.Mode&unix.S_IFMT == unix.S_IFDIR && sharedStat.Mode&unix.S_ISVTX != 0 {
		sharedFD, openErr := unix.Openat(rootFD, ".Trash", unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		if openErr == nil {
			userFD, userErr := ensurePrivateDirectory(sharedFD, uid)
			_ = unix.Close(sharedFD)
			if userErr == nil {
				_ = unix.Close(userFD)
				return desktopTrashLocation{root: volumeRoot, relative: filepath.Join(".Trash", uid), originBase: volumeRoot, volumeRoot: volumeRoot}, nil
			}
		}
	}

	privateName := ".Trash-" + uid
	privateFD, err := ensurePrivateDirectory(rootFD, privateName)
	if err != nil {
		return desktopTrashLocation{}, fmt.Errorf("%w: the removable filesystem has no writable local trash", ErrTrashUnavailable)
	}
	_ = unix.Close(privateFD)
	return desktopTrashLocation{root: volumeRoot, relative: privateName, originBase: volumeRoot, volumeRoot: volumeRoot}, nil
}

func (s *desktopTrashService) trashLocations() ([]desktopTrashLocation, error) {
	locations := make([]desktopTrashLocation, 0)
	seen := make(map[string]struct{})
	dataHome, homeErr := s.dataHome(false)
	if homeErr == nil {
		location := desktopTrashLocation{root: dataHome, relative: "Trash", originBase: dataHome, home: true}
		if dirs, err := s.openTrashDirs(location, false); err == nil {
			dirs.close()
			locations = append(locations, location)
			seen[location.path()] = struct{}{}
		}
	} else if !errors.Is(homeErr, os.ErrNotExist) {
		return nil, fmt.Errorf("%w: home trash directory is unavailable", ErrTrashUnavailable)
	}

	mountInfo, err := s.readMountInfo()
	if err != nil {
		return nil, fmt.Errorf("%w: could not inspect mounted filesystems", ErrTrashUnavailable)
	}
	uid := strconv.Itoa(os.Getuid())
	for _, mount := range mountInfo {
		for _, relative := range []string{filepath.Join(".Trash", uid), ".Trash-" + uid} {
			location := desktopTrashLocation{root: mount.path, relative: relative, originBase: mount.path, volumeRoot: mount.path}
			if _, exists := seen[location.path()]; exists {
				continue
			}
			dirs, err := s.openTrashDirs(location, false)
			if err != nil {
				continue
			}
			dirs.close()
			seen[location.path()] = struct{}{}
			locations = append(locations, location)
		}
	}
	return locations, nil
}

func (s *desktopTrashService) findEntry(ctx context.Context, id string) (desktopTrashEntry, error) {
	if len(id) != sha256.Size*2 {
		return desktopTrashEntry{}, ErrTrashItemNotFound
	}
	entries, err := s.scan(ctx)
	if err != nil {
		return desktopTrashEntry{}, err
	}
	for _, entry := range entries {
		if entry.item.ID == id {
			return entry, nil
		}
	}
	return desktopTrashEntry{}, ErrTrashItemNotFound
}

func trashItemID(location desktopTrashLocation, storageName string) string {
	key := location.path() + "\x00" + storageName
	hash := sha256.Sum256([]byte(key))
	return hex.EncodeToString(hash[:])
}

func (location desktopTrashLocation) path() string {
	return filepath.Join(location.root, location.relative)
}

func (location desktopTrashLocation) volumeRootOrHome() string {
	if location.home {
		return location.root
	}
	return location.volumeRoot
}

func mountInfoForPath(mounts []linuxMountInfo, path string) (linuxMountInfo, error) {
	var best linuxMountInfo
	for _, mount := range mounts {
		if pathWithinRoot(mount.path, path) && len(mount.path) > len(best.path) {
			best = mount
		}
	}
	if best.path == "" {
		return linuxMountInfo{}, os.ErrNotExist
	}
	return best, nil
}

func (s *desktopTrashService) readMountInfo() ([]linuxMountInfo, error) {
	file, err := os.Open(s.cfg.mountInfoPath)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	mounts := make([]linuxMountInfo, 0)
	scanner := bufio.NewScanner(file)
	scanner.Buffer(make([]byte, 4096), 1<<20)
	for scanner.Scan() {
		before, after, ok := strings.Cut(scanner.Text(), " - ")
		if !ok {
			continue
		}
		left := strings.Fields(before)
		right := strings.Fields(after)
		if len(left) < 5 || len(right) < 1 {
			continue
		}
		path := filepath.Clean(decodeMountField(left[4]))
		if !filepath.IsAbs(path) {
			continue
		}
		if resolved, resolveErr := filepath.EvalSymlinks(path); resolveErr == nil {
			path = resolved
		}
		mounts = append(mounts, linuxMountInfo{path: path, fstype: right[0]})
	}
	if err := scanner.Err(); err != nil {
		return nil, err
	}
	return mounts, nil
}

func decodeMountField(value string) string {
	var decoded strings.Builder
	for i := 0; i < len(value); i++ {
		if value[i] == '\\' && i+3 < len(value) {
			switch value[i+1 : i+4] {
			case "040":
				decoded.WriteByte(' ')
				i += 3
				continue
			case "011":
				decoded.WriteByte('\t')
				i += 3
				continue
			case "012":
				decoded.WriteByte('\n')
				i += 3
				continue
			case "134":
				decoded.WriteByte('\\')
				i += 3
				continue
			}
		}
		decoded.WriteByte(value[i])
	}
	return decoded.String()
}

func isRemoteFilesystem(fstype string) bool {
	switch strings.ToLower(fstype) {
	case "nfs", "nfs4", "cifs", "smb3", "davfs", "sshfs", "9p",
		"fuse.sshfs", "fuse.rclone", "fuse.s3fs", "fuse.davfs", "fuse.curlftpfs":
		return true
	default:
		return false
	}
}
