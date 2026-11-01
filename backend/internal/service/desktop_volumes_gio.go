package service

import (
	"strconv"
	"strings"
)

// gio mount --list --detail prints an indented tree:
//
//	Drive(0): USB DISK
//	  ids:
//	   unix-device: '/dev/sdb'
//	  is_removable=1
//	  Volume(0): Photos
//	    ids:
//	     unix-device: '/dev/sdb1'
//	     uuid: '1234-ABCD'
//	    can_mount=0
//	    Mount(0): Photos -> file:///run/media/user/Photos
//	      can_unmount=1
//	      can_eject=1
//
// Top-level volumes and mounts (those without a drive) are not removable
// drives and are ignored here.
type gioVolumeListing struct {
	drives []gioDrive
}

type gioDrive struct {
	name           string
	ids            map[string]string
	removable      bool
	mediaRemovable bool
	hasMedia       bool
	canEject       bool
	startStopType  string
	volumes        []gioVolume
}

type gioVolume struct {
	name     string
	ids      map[string]string
	uuid     string
	canMount bool
	canEject bool
	mount    *gioMount
}

type gioMount struct {
	uri        string
	canUnmount bool
	canEject   bool
}

type gioVolumeRef struct {
	drive  *gioDrive
	volume *gioVolume
}

type gioIdentifierTarget interface {
	setIdentifier(key, value string)
}

func (d *gioDrive) setIdentifier(key, value string) {
	d.ids[key] = value
}

func (v *gioVolume) setIdentifier(key, value string) {
	v.ids[key] = value
	if key == "uuid" {
		v.uuid = value
	}
}

func parseGioMountList(output []byte) gioVolumeListing {
	var listing gioVolumeListing
	var drive *gioDrive
	var volume *gioVolume
	var mount *gioMount
	var driveIndent, volumeIndent, mountIndent int
	var idsIndent int
	var idsTarget gioIdentifierTarget

	resetIDs := func() {
		idsTarget = nil
		idsIndent = 0
	}

	for _, raw := range strings.Split(string(output), "\n") {
		line := strings.TrimRight(raw, " \t\r")
		if strings.TrimSpace(line) == "" {
			continue
		}
		indent := len(line) - len(strings.TrimLeft(line, " "))
		trimmed := strings.TrimLeft(line, " ")

		if name, ok := gioHeader(trimmed, "Drive"); ok && indent == 0 {
			listing.drives = append(listing.drives, gioDrive{name: name, ids: map[string]string{}})
			drive = &listing.drives[len(listing.drives)-1]
			driveIndent = indent
			volume, mount = nil, nil
			resetIDs()
			continue
		}
		if name, ok := gioHeader(trimmed, "Volume"); ok {
			if drive != nil && indent > driveIndent {
				drive.volumes = append(drive.volumes, gioVolume{name: name, ids: map[string]string{}})
				volume = &drive.volumes[len(drive.volumes)-1]
				volumeIndent = indent
				mount = nil
			} else {
				// A volume without a drive cannot be identified as removable.
				drive, volume, mount = nil, nil, nil
			}
			resetIDs()
			continue
		}
		if uri, ok := gioMountHeader(trimmed); ok {
			if volume != nil && indent > volumeIndent {
				volume.mount = &gioMount{uri: uri}
				mount = volume.mount
				mountIndent = indent
			} else {
				// Top-level mounts repeat the nested mounts and carry no drive.
				mount = nil
			}
			resetIDs()
			continue
		}
		if trimmed == "ids:" {
			idsIndent = indent
			switch {
			case volume != nil && indent == volumeIndent+2:
				idsTarget = volume
			case drive != nil && indent == driveIndent+2:
				idsTarget = drive
			default:
				idsTarget = nil
			}
			continue
		}
		if idsTarget != nil && indent == idsIndent+1 {
			if key, value, ok := strings.Cut(trimmed, ": "); ok {
				idsTarget.setIdentifier(key, strings.Trim(value, "'"))
				continue
			}
		}

		key, value, ok := strings.Cut(trimmed, "=")
		if !ok {
			continue
		}
		switch {
		case volume != nil && indent == volumeIndent+2:
			applyVolumeProperty(volume, key, value)
		case drive != nil && indent == driveIndent+2:
			applyDriveProperty(drive, key, value)
		case mount != nil && indent == mountIndent+2:
			applyMountProperty(mount, key, value)
		}
	}
	return listing
}

func gioHeader(line, kind string) (string, bool) {
	prefix := kind + "("
	if !strings.HasPrefix(line, prefix) {
		return "", false
	}
	rest := line[len(prefix):]
	close := strings.Index(rest, "): ")
	if close < 0 {
		return "", false
	}
	if _, err := strconv.Atoi(rest[:close]); err != nil {
		return "", false
	}
	return rest[close+3:], true
}

func gioMountHeader(line string) (string, bool) {
	payload, ok := gioHeader(line, "Mount")
	if !ok {
		return "", false
	}
	separator := strings.LastIndex(payload, " -> ")
	if separator < 0 {
		return "", true
	}
	return payload[separator+4:], true
}

func applyDriveProperty(drive *gioDrive, key, value string) {
	switch key {
	case "is_removable":
		drive.removable = value == "1"
	case "is_media_removable":
		drive.mediaRemovable = value == "1"
	case "has_media":
		drive.hasMedia = value == "1"
	case "can_eject":
		drive.canEject = value == "1"
	case "start_stop_type":
		drive.startStopType = value
	}
}

func applyVolumeProperty(volume *gioVolume, key, value string) {
	switch key {
	case "can_mount":
		volume.canMount = value == "1"
	case "can_eject":
		volume.canEject = value == "1"
	}
}

func applyMountProperty(mount *gioMount, key, value string) {
	switch key {
	case "can_unmount":
		mount.canUnmount = value == "1"
	case "can_eject":
		mount.canEject = value == "1"
	}
}

func (listing gioVolumeListing) removableVolumes() []gioVolumeRef {
	var volumes []gioVolumeRef
	for index := range listing.drives {
		drive := &listing.drives[index]
		if !drive.removable && !drive.mediaRemovable && !drive.canEject && drive.startStopType != "external" {
			continue
		}
		if !drive.hasMedia && len(drive.volumes) == 0 {
			continue
		}
		for volumeIndex := range drive.volumes {
			volumes = append(volumes, gioVolumeRef{drive: drive, volume: &drive.volumes[volumeIndex]})
		}
	}
	return volumes
}
