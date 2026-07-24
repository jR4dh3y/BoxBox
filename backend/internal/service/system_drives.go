package service

import (
	"bufio"
	"os"
	"os/exec"
	"strconv"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/config"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

// systemMountInfo stores information about a mount point from /proc/mounts
type systemMountInfo struct {
	Device     string
	MountPoint string
	FSType     string
}

// getSystemMountInfo reads /proc/mounts to get mount information
func getSystemMountInfo() ([]systemMountInfo, error) {
	file, err := os.Open(config.ProcMountsPath)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	var mounts []systemMountInfo
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) >= 3 {
			mounts = append(mounts, systemMountInfo{
				Device:     fields[0],
				MountPoint: fields[1],
				FSType:     fields[2],
			})
		}
	}
	return mounts, scanner.Err()
}

// getAllSystemDrives uses the df command to get all mounted filesystems
func getAllSystemDrives() ([]model.SystemDrive, error) {
	// Run df with POSIX output format for consistent parsing
	// -P: POSIX format (ensures no line wrapping)
	// -B1: Block size of 1 byte (gives us exact byte counts)
	cmd := exec.Command("df", "-P", "-B1")
	output, err := cmd.Output()
	if err != nil {
		// Fallback: try with -k if -B1 is not supported
		cmd = exec.Command("df", "-P", "-k")
		output, err = cmd.Output()
		if err != nil {
			return nil, err
		}
		return parseDfOutput(output, true)
	}
	return parseDfOutput(output, false)
}

// parseDfOutput parses the output of the df command
// kilobytes: if true, values are in 1K blocks; otherwise in bytes
func parseDfOutput(output []byte, kilobytes bool) ([]model.SystemDrive, error) {
	var drives []model.SystemDrive
	scanner := bufio.NewScanner(strings.NewReader(string(output)))

	// Skip header line
	if scanner.Scan() {
		// Header: Filesystem 1B-blocks Used Available Use% Mounted on
	}

	// Get mount info for filesystem types
	mountInfoMap := buildSystemMountInfoMap()

	for scanner.Scan() {
		line := scanner.Text()
		fields := strings.Fields(line)

		// df -P format: Filesystem 1B-blocks Used Available Capacity Mounted_on
		// Minimum 6 fields required
		if len(fields) < 6 {
			continue
		}

		device := fields[0]
		totalStr := fields[1]
		usedStr := fields[2]
		freeStr := fields[3]
		// fields[4] is percentage with %
		mountPoint := fields[5]

		// Handle mount points with spaces by joining remaining fields
		if len(fields) > 6 {
			mountPoint = strings.Join(fields[5:], " ")
		}

		// Parse numeric values
		total, err := strconv.ParseUint(totalStr, 10, 64)
		if err != nil {
			continue
		}
		used, err := strconv.ParseUint(usedStr, 10, 64)
		if err != nil {
			continue
		}
		free, err := strconv.ParseUint(freeStr, 10, 64)
		if err != nil {
			continue
		}

		// Convert from kilobytes if needed (macOS df -k)
		if kilobytes {
			total *= 1024
			used *= 1024
			free *= 1024
		}

		// Get filesystem type from mount info
		fsType := ""
		if info, ok := mountInfoMap[mountPoint]; ok {
			fsType = info.FSType
		}

		// Skip virtual filesystems
		if config.VirtualFilesystems[fsType] {
			continue
		}

		// Skip pseudo filesystems that don't have a device path
		if strings.HasPrefix(device, "none") || device == "tmpfs" || device == "devtmpfs" {
			continue
		}

		// Apply storage device filtering
		if !isStorageDevice(mountPoint, device, fsType) {
			continue
		}

		// Calculate usage percentage
		usedPct := float64(0)
		if total > 0 {
			usedPct = float64(used) / float64(total) * config.PercentMultiplier
		}

		drives = append(drives, model.SystemDrive{
			Device:     device,
			MountPoint: mountPoint,
			FSType:     fsType,
			TotalBytes: total,
			UsedBytes:  used,
			FreeBytes:  free,
			UsedPct:    usedPct,
		})
	}

	return drives, scanner.Err()
}

// isStorageDevice checks if a mount point represents a real storage device
// that should be shown to the user (not Docker bind mounts, system paths, etc.)
func isStorageDevice(mountPoint, device, fsType string) bool {
	// Root filesystem is always included
	if mountPoint == "/" {
		return true
	}

	// Check against excluded prefixes first
	for _, prefix := range config.ExcludedMountPointPrefixes {
		if strings.HasPrefix(mountPoint, prefix) {
			return false
		}
	}

	// Check against excluded suffixes
	for _, suffix := range config.ExcludedMountPointSuffixes {
		if strings.HasSuffix(mountPoint, suffix) {
			return false
		}
	}

	// Only allow mounts that look like actual storage locations
	isAllowed := false
	for _, prefix := range config.AllowedMountPointPrefixes {
		if prefix == "/" {
			// Skip root check here as it's handled above
			continue
		}
		if strings.HasPrefix(mountPoint, prefix) {
			isAllowed = true
			break
		}
	}

	if !isAllowed {
		return false
	}

	// Additional check: skip very small partitions (likely boot/EFI partitions)
	// This is handled via mount point suffixes above, but we can add more logic if needed

	// Skip bind mounts to single files (these show up in Docker)
	// They typically don't have a real block device
	if !strings.HasPrefix(device, "/dev/") {
		// Allow network filesystems and FUSE mounts
		if fsType != "nfs" && fsType != "nfs4" && fsType != "cifs" && fsType != "smb" &&
			!strings.HasPrefix(fsType, "fuse") {
			return false
		}
	}

	return true
}

// buildSystemMountInfoMap creates a map of mount point to mount info for fstype lookup
func buildSystemMountInfoMap() map[string]systemMountInfo {
	mounts, err := getSystemMountInfo()
	if err != nil {
		return make(map[string]systemMountInfo)
	}

	result := make(map[string]systemMountInfo, len(mounts))
	for _, m := range mounts {
		result[m.MountPoint] = m
	}
	return result
}
