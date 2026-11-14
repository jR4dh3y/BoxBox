package main

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

const (
	portalBus       = "org.freedesktop.impl.portal.desktop.boxbox"
	portalInterface = "org.freedesktop.impl.portal.FileChooser"
	portalMarker    = "# BoxBox file chooser override"
)

var portalAssignment = regexp.MustCompile(`^\s*` + regexp.QuoteMeta(portalInterface) + `\s*=`)

// chooserRouting changes only FileChooser in a portals.conf, reversibly: the
// previous line is kept in a comment and restored on disable.
func chooserRouting(source string, enable bool) (string, error) {
	lines := strings.Split(source, "\n")
	for i, line := range lines {
		if line != portalMarker {
			continue
		}
		if i+2 >= len(lines) || !strings.HasPrefix(lines[i+1], "# Previous: ") || lines[i+2] != portalInterface+"=boxbox" {
			return "", errors.New("the managed chooser routing was edited; restore it manually before changing it")
		}
		previous := strings.TrimPrefix(lines[i+1], "# Previous: ")
		restored := []string{}
		if previous != "" {
			restored = append(restored, previous)
		}
		lines = append(append(append([]string{}, lines[:i]...), restored...), lines[i+3:]...)
		break
	}
	if !enable {
		return strings.Join(lines, "\n"), nil
	}
	group, insertion, previous := false, -1, ""
	for i, line := range lines {
		if strings.HasPrefix(strings.TrimSpace(line), "[") {
			if group && insertion < 0 {
				insertion = i
			}
			group = strings.TrimSpace(line) == "[preferred]"
		}
		if group && portalAssignment.MatchString(line) {
			if previous != "" {
				return "", errors.New("duplicate FileChooser routing")
			}
			previous, insertion = line, i
		}
	}
	if insertion < 0 {
		hasGroup := false
		for _, line := range lines {
			hasGroup = hasGroup || strings.TrimSpace(line) == "[preferred]"
		}
		if !hasGroup {
			lines = append(lines, "[preferred]")
		}
		insertion = len(lines)
	}
	managed := []string{portalMarker, "# Previous: " + previous, portalInterface + "=boxbox"}
	skip := 0
	if previous != "" {
		skip = 1
	}
	out := append(append(append([]string{}, lines[:insertion]...), managed...), lines[insertion+skip:]...)
	return strings.Join(out, "\n"), nil
}

// quoteExecArgument quotes one argument for a Desktop Entry Exec line.
func quoteExecArgument(value string) (string, error) {
	if value == "" {
		return "", errors.New("desktop entry arguments cannot be empty")
	}
	for _, r := range value {
		if r < 0x20 || r == 0x7f {
			return "", errors.New("desktop entry arguments cannot contain control characters")
		}
	}
	escaped := regexp.MustCompile("[\\\\\"`$]").ReplaceAllString(value, `\$0`)
	return `"` + escaped + `"`, nil
}

// readPortalFile reads a small regular file; a missing one is "", false.
func readPortalFile(path string) (string, bool, error) {
	info, err := os.Lstat(path)
	if errors.Is(err, fs.ErrNotExist) {
		return "", false, nil
	}
	if err != nil {
		return "", false, err
	}
	if !info.Mode().IsRegular() || info.Size() > 128<<10 {
		return "", false, fmt.Errorf("refusing unsafe portal file: %s", path)
	}
	data, err := os.ReadFile(path)
	return string(data), err == nil, err
}

func writeAtomic(path, content string) error {
	if _, _, err := readPortalFile(path); err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	temporary := fmt.Sprintf("%s.boxbox-%d", path, os.Getpid())
	file, err := os.OpenFile(temporary, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return err
	}
	_, err = file.WriteString(content)
	if closeErr := file.Close(); err == nil {
		err = closeErr
	}
	if err == nil {
		err = os.Rename(temporary, path)
	}
	if err != nil {
		_ = os.Remove(temporary)
	}
	return err
}

// installChooserPortal routes the desktop's FileChooser portal to BoxBox, or
// back. getenv reads the environment.
func installChooserPortal(launcher string, enable bool, getenv func(string) string) (string, error) {
	location := func(key, fallback string) (string, error) {
		value := getenv(key)
		if value == "" {
			value = fallback
		}
		if !filepath.IsAbs(value) {
			return "", fmt.Errorf("%s must be absolute", key)
		}
		return value, nil
	}
	home := getenv("HOME")
	if home == "" {
		var err error
		if home, err = os.UserHomeDir(); err != nil {
			return "", err
		}
	}
	config, err := location("XDG_CONFIG_HOME", filepath.Join(home, ".config"))
	if err != nil {
		return "", err
	}
	data, err := location("XDG_DATA_HOME", filepath.Join(home, ".local/share"))
	if err != nil {
		return "", err
	}
	var names []string
	for _, desktop := range strings.Split(getenv("XDG_CURRENT_DESKTOP"), ":") {
		if desktop == "" {
			continue
		}
		desktop = strings.ToLower(desktop)
		if !regexp.MustCompile(`^[a-z0-9_-]+$`).MatchString(desktop) {
			return "", errors.New("invalid desktop name")
		}
		names = append(names, desktop+"-portals.conf")
	}
	names = append(names, "portals.conf")
	split := func(key, fallback string) []string {
		if value := getenv(key); value != "" {
			return strings.Split(value, ":")
		}
		return strings.Split(fallback, ":")
	}
	roots := append(append([]string{config}, split("XDG_CONFIG_DIRS", "/etc/xdg")...), "/etc", data)
	roots = append(roots, split("XDG_DATA_DIRS", "/usr/local/share:/usr/share")...)
	source, sourcePath := "", ""
	for _, root := range roots {
		for _, name := range names {
			path := filepath.Join(root, "xdg-desktop-portal", name)
			content, ok, err := readPortalFile(path)
			if err != nil {
				return "", err
			}
			if ok {
				source, sourcePath = content, path
				break
			}
		}
		if sourcePath != "" {
			break
		}
	}
	if !enable && !strings.Contains(source, portalMarker) {
		return "", errors.New("BoxBox has no managed chooser override in the effective configuration; no defaults changed")
	}
	if enable {
		// The portal discovers .portal metadata in its compiled-in directory unless overridden.
		metadata, err := location("XDG_DESKTOP_PORTAL_DIR", "/usr/share/xdg-desktop-portal/portals")
		if err != nil {
			return "", err
		}
		if _, ok, err := readPortalFile(filepath.Join(metadata, "boxbox.portal")); err != nil || !ok {
			return "", errors.New("install boxbox.portal in the system portal metadata directory first; no defaults changed")
		}
		if !filepath.IsAbs(launcher) {
			return "", errors.New("the portal launcher must be absolute")
		}
		service := filepath.Join(data, "dbus-1/services", portalBus+".service")
		old, ok, err := readPortalFile(service)
		if err != nil {
			return "", err
		}
		if ok && !strings.Contains(old, "# BoxBox managed") {
			return "", errors.New("refusing to overwrite an unmanaged D-Bus service")
		}
		exec, err := quoteExecArgument(launcher)
		if err != nil {
			return "", err
		}
		if err := writeAtomic(service, fmt.Sprintf("# BoxBox managed\n[D-BUS Service]\nName=%s\nExec=%s --portal\n", portalBus, exec)); err != nil {
			return "", err
		}
	}
	destination := filepath.Join(config, "xdg-desktop-portal", names[0])
	if strings.HasPrefix(sourcePath, config+"/") {
		destination = sourcePath
	}
	routed, err := chooserRouting(source, enable)
	if err != nil {
		return "", err
	}
	return destination, writeAtomic(destination, routed)
}
