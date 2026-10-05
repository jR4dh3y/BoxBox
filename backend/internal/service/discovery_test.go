//go:build linux

package service

import (
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestDiscoveredDrivesOpenThroughParentMount(t *testing.T) {
	fs := filesystem.NewMemMapFS()
	_ = fs.MkdirAll("/media/devmon/SanDisk", 0o755)
	_ = fs.MkdirAll("/media/devmon/not-mounted", 0o755)
	entries, err := fs.ReadDir("/media/devmon")
	if err != nil {
		t.Fatal(err)
	}

	parent := model.MountPoint{Name: "drives", Path: "/media/devmon", AutoDiscover: true}
	mountSet := map[string]mountInfo{normalizePath("/media/devmon/SanDisk"): {}}
	discovered := filterMountedDirs(entries, parent, mountSet)

	if len(discovered) != 1 {
		t.Fatalf("discovered %d drives, want 1: %+v", len(discovered), discovered)
	}
	drive := discovered[0]
	if drive.Name != "drives/SanDisk" || drive.Path != "/media/devmon/SanDisk" || drive.Kind != model.MountKindDrive {
		t.Fatalf("discovered drive = %+v", drive)
	}
}
