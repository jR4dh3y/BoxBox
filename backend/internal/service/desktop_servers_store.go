package service

import (
	"crypto/rand"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"uuid"
)

type desktopServerSnapshot struct {
	Version int                            `json:"version"`
	Servers map[string]desktopServerRecord `json:"servers"`
}

func (s *DesktopServers) load(dataDir string) error {
	directory := filepath.Join(dataDir, "desktop")
	if err := s.fs.MkdirAll(directory, 0700); err != nil {
		return err
	}
	info, err := s.fs.Lstat(directory)
	if err != nil {
		return err
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return ErrDesktopServerStorage
	}
	if err := s.fs.Chmod(directory, 0700); err != nil {
		return err
	}
	s.path = filepath.Join(directory, "servers.json")
	info, err = s.fs.Lstat(s.path)
	if errors.Is(err, os.ErrNotExist) {
		return s.save(nil)
	}
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() || info.Size() > 1<<20 {
		return ErrDesktopServerStorage
	}
	if err := s.fs.Chmod(s.path, 0600); err != nil {
		return err
	}
	file, err := s.fs.Open(s.path)
	if err != nil {
		return err
	}
	defer file.Close()
	decoder := json.NewDecoder(io.LimitReader(file, 1<<20))
	decoder.DisallowUnknownFields()
	var snapshot desktopServerSnapshot
	if err := decoder.Decode(&snapshot); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		return ErrDesktopServerStorage
	}
	if snapshot.Version != 1 || snapshot.Servers == nil || len(snapshot.Servers) > 100 {
		return ErrDesktopServerStorage
	}
	for id, record := range snapshot.Servers {
		parsed, err := uuid.Parse(id)
		if err != nil || parsed.String() != id || id != record.ID || !validDesktopName(record.Name) {
			return ErrDesktopServerStorage
		}
		u, err := desktopServerURL(record.URL)
		if err != nil || u.String() != record.URL {
			return ErrDesktopServerStorage
		}
		if record.ExpiresAt.IsZero() || record.ClientID == "" || len(record.ClientID) > 128 || !validDesktopCredential(record.Credential) {
			return ErrDesktopServerStorage
		}
	}
	s.records = snapshot.Servers
	return nil
}

func (s *DesktopServers) cloneRecords() map[string]desktopServerRecord {
	result := make(map[string]desktopServerRecord, len(s.records))
	for id, record := range s.records {
		result[id] = record
	}
	return result
}

// After publication fails to sync, fail closed until restart.
func (s *DesktopServers) save(previous map[string]desktopServerRecord) error {
	if s.failed {
		return ErrDesktopServerStorage
	}
	published, err := s.writeSnapshot()
	if err == nil {
		return nil
	}
	if published {
		s.failed = true
	} else if previous != nil {
		s.records = previous
	}
	return ErrDesktopServerStorage
}

func (s *DesktopServers) writeSnapshot() (published bool, err error) {
	temporary := s.path + ".tmp." + rand.Text()
	file, err := s.fs.OpenFile(temporary, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
	if err != nil {
		return false, err
	}
	closed := false
	defer func() {
		if !closed {
			file.Close()
		}
		if !published {
			s.fs.Remove(temporary)
		}
	}()
	if err := json.NewEncoder(file).Encode(desktopServerSnapshot{Version: 1, Servers: s.records}); err != nil {
		return false, err
	}
	if err := file.Sync(); err != nil {
		return false, err
	}
	err = file.Close()
	closed = true
	if err != nil {
		return false, err
	}
	if err := s.fs.Rename(temporary, s.path); err != nil {
		return false, err
	}
	directory, err := s.fs.Open(filepath.Dir(s.path))
	if err != nil {
		return true, err
	}
	syncErr := directory.Sync()
	closeErr := directory.Close()
	return true, errors.Join(syncErr, closeErr)
}
