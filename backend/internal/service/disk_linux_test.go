package service

import (
	"strings"
	"testing"
)

func TestParseHostMounts(t *testing.T) {
	table := strings.Join([]string{
		`22 1 8:2 / / rw,relatime shared:1 - ext4 /dev/sda2 rw`,
		`40 22 0:45 /@home /home rw,relatime shared:2 - btrfs /dev/sdb1 rw,subvol=/@home`,
		`41 22 8:2 /srv/my\040files /mnt/my\040files rw - ext4 /dev/sda2 rw`,
		`malformed line`,
	}, "\n")
	mounts, err := parseHostMounts(strings.NewReader(table))
	if err != nil {
		t.Fatal(err)
	}
	want := []hostMount{
		{path: "/", device: "8:2", root: "/"},
		{path: "/home", device: "0:45", root: "/@home"},
		{path: "/mnt/my files", device: "8:2", root: "/srv/my files"},
	}
	if len(mounts) != len(want) {
		t.Fatalf("parseHostMounts() = %+v, want %+v", mounts, want)
	}
	for i := range want {
		if mounts[i] != want[i] {
			t.Errorf("mount %d = %+v, want %+v", i, mounts[i], want[i])
		}
	}
}
