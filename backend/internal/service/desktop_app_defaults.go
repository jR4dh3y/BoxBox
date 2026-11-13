package service

import (
	"os"
	"path/filepath"
	"strings"
)

// GIO writes generic mimeapps.list, but an existing user desktop-specific default
// takes precedence. Change only that MIME key, retaining comments and other settings.
func updateDesktopMIMEOverride(mimeType, appID string) error {
	configHome := xdgHomeDirectory("XDG_CONFIG_HOME", ".config")
	for _, path := range mimeAppsFiles() {
		if filepath.Dir(path) != configHome || filepath.Base(path) == "mimeapps.list" {
			break
		}
		if len(readMIMEAssociations(mimeType, []string{path}).defaults) == 0 {
			continue
		}
		// Preserve a user's symlink rather than replacing the link itself.
		target, err := filepath.EvalSymlinks(path)
		if err != nil {
			return err
		}
		data, err := os.ReadFile(target)
		if err != nil {
			return err
		}
		lines := strings.Split(string(data), "\n")
		inDefaults := false
		for i, line := range lines {
			line = strings.TrimSpace(line)
			if strings.HasPrefix(line, "[") && strings.HasSuffix(line, "]") {
				inDefaults = line == "[Default Applications]"
				continue
			}
			key, value, ok := strings.Cut(line, "=")
			if !inDefaults || !ok || key != mimeType {
				continue
			}
			ids := []string{appID}
			for _, id := range desktopIDs(value) {
				if id != appID {
					ids = append(ids, id)
				}
			}
			lines[i] = mimeType + "=" + strings.Join(ids, ";") + ";"
		}
		return replaceMIMEConfig(target, []byte(strings.Join(lines, "\n")))
	}
	return nil
}

func replaceMIMEConfig(path string, data []byte) error {
	info, err := os.Stat(path)
	if err != nil {
		return err
	}
	file, err := os.CreateTemp(filepath.Dir(path), ".boxbox-mimeapps-*")
	if err != nil {
		return err
	}
	defer os.Remove(file.Name())
	defer file.Close()
	if err := file.Chmod(info.Mode().Perm()); err != nil {
		return err
	}
	if _, err := file.Write(data); err != nil {
		return err
	}
	if err := file.Sync(); err != nil {
		return err
	}
	if err := file.Close(); err != nil {
		return err
	}
	return os.Rename(file.Name(), path)
}
