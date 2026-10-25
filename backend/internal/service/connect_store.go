package service

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

type connectSnapshot struct {
	Version int                      `json:"version"`
	Clients map[string]connectRecord `json:"clients"`
}

func (s *ConnectService) loadClients() error {
	directory := filepath.Dir(s.path)
	if err := s.fs.MkdirAll(directory, 0o700); err != nil {
		return err
	}
	info, err := s.fs.Lstat(directory)
	if err != nil {
		return err
	}
	if !info.IsDir() || info.Mode()&os.ModeSymlink != 0 {
		return errors.New("connected client directory must be a real directory")
	}
	if err := s.fs.Chmod(directory, 0o700); err != nil {
		return err
	}
	if err := syncDirectory(s.fs, filepath.Dir(directory)); err != nil {
		return err
	}
	info, err = s.fs.Lstat(s.path)
	if errors.Is(err, os.ErrNotExist) {
		return s.saveClients(nil)
	}
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() {
		return errors.New("connected client store must be a regular file")
	}
	if err := s.fs.Chmod(s.path, 0o600); err != nil {
		return err
	}
	file, err := s.fs.Open(s.path)
	if err != nil {
		return err
	}
	defer file.Close()
	// Configured usernames have no size limit; read every snapshot we can persist.
	decoder := json.NewDecoder(file)
	decoder.DisallowUnknownFields()
	var snapshot connectSnapshot
	if err := decoder.Decode(&snapshot); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		return errors.New("connected client store contains trailing data")
	}
	if snapshot.Version != 1 || snapshot.Clients == nil || len(snapshot.Clients) > maxConnectClients {
		return errors.New("invalid connected client snapshot")
	}
	hashes := make(map[string]bool, len(snapshot.Clients))
	for id, record := range snapshot.Clients {
		digest, err := hex.DecodeString(record.CredentialHash)
		if id == "" || len(id) > 100 || id != record.ClientID || !validClientName(record.ClientName) ||
			strings.TrimSpace(record.Username) == "" || record.CreatedAt.IsZero() || !record.ExpiresAt.After(record.CreatedAt) ||
			err != nil || len(digest) != 32 || hex.EncodeToString(digest) != record.CredentialHash || hashes[record.CredentialHash] {
			return errors.New("invalid connected client record")
		}
		hashes[record.CredentialHash] = true
	}
	s.clients = snapshot.Clients
	return nil
}

// A publication followed by failed directory sync has uncertain durability.
// Reject all paired access until restart rather than rolling back a published revoke.
func (s *ConnectService) saveClients(previous map[string]connectRecord) error {
	published, err := s.writeClients()
	if err == nil {
		return nil
	}
	if published {
		s.storeFailed = true
	} else if previous != nil {
		s.clients = previous
	}
	return errors.Join(ErrConnectStorage, err)
}

func (s *ConnectService) writeClients() (published bool, err error) {
	temporary := s.path + ".tmp." + rand.Text()
	file, err := s.fs.OpenFile(temporary, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
	if err != nil {
		return false, fmt.Errorf("create connected client snapshot: %w", err)
	}
	closed := false
	defer func() {
		if !closed {
			_ = file.Close()
		}
		if !published {
			_ = s.fs.Remove(temporary)
		}
	}()
	if err := s.fs.Chmod(temporary, 0o600); err != nil {
		return false, err
	}
	if err := json.NewEncoder(file).Encode(connectSnapshot{Version: 1, Clients: s.clients}); err != nil {
		return false, err
	}
	if err := file.Sync(); err != nil {
		return false, err
	}
	if err := file.Close(); err != nil {
		closed = true
		return false, err
	}
	closed = true
	if err := s.fs.Rename(temporary, s.path); err != nil {
		return false, err
	}
	return true, syncDirectory(s.fs, filepath.Dir(s.path))
}
