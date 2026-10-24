package service

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/url"
	"sort"
	"strings"
	"sync"
	"time"
	"unicode"
	"unicode/utf8"
	"uuid"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

var (
	ErrDesktopServerNotFound = errors.New("saved server not found")
	ErrDesktopServerInput    = errors.New("invalid saved server input")
	ErrDesktopServerStorage  = errors.New("saved server storage unavailable")
	ErrRemoteUnavailable     = errors.New("remote server unavailable")
	ErrRemoteAuthRequired    = errors.New("remote server requires pairing")
	ErrRemotePairing         = errors.New("remote pairing code invalid or unavailable")
	ErrRemoteRoute           = errors.New("remote route is not allowed")
	ErrCrossSourceMove       = errors.New("cross-source move is unsupported; use copy")
)

type DesktopServer struct {
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	URL       string    `json:"url"`
	ExpiresAt time.Time `json:"expiresAt"`
}

type desktopServerRecord struct {
	DesktopServer
	ClientID   string `json:"clientId"`
	Credential string `json:"credential"`
}

type DesktopServers struct {
	mu      sync.RWMutex
	records map[string]desktopServerRecord
	fs      filesystem.FS
	path    string
	client  *http.Client
	failed  bool
}

func NewDesktopServers(fsys filesystem.FS, dataDir string, client *http.Client) (*DesktopServers, error) {
	if strings.TrimSpace(dataDir) == "" {
		return nil, ErrDesktopServerStorage
	}
	c := http.Client{}
	if client != nil {
		c = *client
	} else {
		transport := http.DefaultTransport.(*http.Transport).Clone()
		transport.ResponseHeaderTimeout = 30 * time.Second
		c.Transport = transport
	}
	c.Jar = nil
	c.CheckRedirect = func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }
	s := &DesktopServers{fs: fsys, client: &c, records: make(map[string]desktopServerRecord)}
	if err := s.load(dataDir); err != nil {
		return nil, ErrDesktopServerStorage
	}
	return s, nil
}

func validDesktopName(name string) bool {
	if strings.TrimSpace(name) != name || utf8.RuneCountInString(name) < 1 || utf8.RuneCountInString(name) > 80 {
		return false
	}
	for _, r := range name {
		if unicode.IsControl(r) {
			return false
		}
	}
	return true
}

func desktopServerURL(raw string) (*url.URL, error) {
	if len(raw) > 4096 {
		return nil, ErrDesktopServerInput
	}
	u, err := url.Parse(raw)
	if err != nil || u.Scheme != "https" || u.Hostname() == "" || u.User != nil || u.RawQuery != "" || u.ForceQuery || u.Fragment != "" || u.Opaque != "" || u.RawPath != "" || strings.Contains(u.Host, "%") {
		return nil, ErrDesktopServerInput
	}
	u.Path = strings.TrimRight(u.Path, "/")
	if u.Path != "" && !safeClientRoutePath(u.Path) {
		return nil, ErrDesktopServerInput
	}
	return u, nil
}

// Available reports durable-storage failures without exposing credentials.
func (s *DesktopServers) Available() bool {
	if s == nil {
		return false
	}
	s.mu.RLock()
	defer s.mu.RUnlock()
	return !s.failed
}

func (s *DesktopServers) List() []DesktopServer {
	s.mu.RLock()
	defer s.mu.RUnlock()
	result := make([]DesktopServer, 0, len(s.records))
	for _, r := range s.records {
		result = append(result, r.DesktopServer)
	}
	sort.Slice(result, func(i, j int) bool { return result[i].ID < result[j].ID })
	return result
}

func (s *DesktopServers) record(id string) (desktopServerRecord, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	if s.failed {
		return desktopServerRecord{}, ErrDesktopServerStorage
	}
	r, ok := s.records[id]
	if !ok {
		return r, ErrDesktopServerNotFound
	}
	return r, nil
}

func (s *DesktopServers) Create(ctx context.Context, name, rawURL, code, clientName string) (*DesktopServer, error) {
	name = strings.TrimSpace(name)
	if !validDesktopName(name) {
		return nil, ErrDesktopServerInput
	}
	u, err := desktopServerURL(rawURL)
	if err != nil {
		return nil, err
	}
	record, err := s.redeem(ctx, u, code, clientName)
	if err != nil {
		return nil, err
	}
	record.DesktopServer.ID = uuid.New().String()
	record.Name = name
	record.URL = u.String()
	s.mu.Lock()
	if s.failed || len(s.records) >= 100 {
		s.mu.Unlock()
		s.revoke(ctx, record)
		return nil, ErrDesktopServerStorage
	}
	previous := s.cloneRecords()
	s.records[record.ID] = record
	err = s.save(previous)
	s.mu.Unlock()
	if err != nil {
		s.revoke(ctx, record)
		return nil, err
	}
	public := record.DesktopServer
	return &public, nil
}

func (s *DesktopServers) Pair(ctx context.Context, id, code, clientName string) (*DesktopServer, error) {
	old, err := s.record(id)
	if err != nil {
		return nil, err
	}
	u, err := desktopServerURL(old.URL)
	if err != nil {
		return nil, err
	}
	record, err := s.redeem(ctx, u, code, clientName)
	if err != nil {
		return nil, err
	}
	s.mu.Lock()
	current, ok := s.records[id]
	if !ok || current.Credential != old.Credential || s.failed {
		s.mu.Unlock()
		s.revoke(ctx, record)
		return nil, ErrDesktopServerStorage
	}
	previous := s.cloneRecords()
	expiry := record.ExpiresAt
	record.DesktopServer = current.DesktopServer
	record.ExpiresAt = expiry
	s.records[id] = record
	err = s.save(previous)
	s.mu.Unlock()
	if err != nil {
		s.revoke(ctx, record)
		return nil, err
	}
	s.revoke(ctx, old)
	public := record.DesktopServer
	return &public, nil
}

func (s *DesktopServers) Rename(id, name string) (*DesktopServer, error) {
	name = strings.TrimSpace(name)
	if !validDesktopName(name) {
		return nil, ErrDesktopServerInput
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	r, ok := s.records[id]
	if !ok {
		return nil, ErrDesktopServerNotFound
	}
	if s.failed {
		return nil, ErrDesktopServerStorage
	}
	previous := s.cloneRecords()
	r.Name = name
	s.records[id] = r
	if err := s.save(previous); err != nil {
		return nil, err
	}
	public := r.DesktopServer
	return &public, nil
}

func (s *DesktopServers) Delete(ctx context.Context, id string) error {
	s.mu.Lock()
	r, ok := s.records[id]
	if !ok {
		s.mu.Unlock()
		return ErrDesktopServerNotFound
	}
	if s.failed {
		s.mu.Unlock()
		return ErrDesktopServerStorage
	}
	previous := s.cloneRecords()
	delete(s.records, id)
	err := s.save(previous)
	s.mu.Unlock()
	if err == nil {
		s.revoke(ctx, r)
	}
	return err
}

func (s *DesktopServers) redeem(ctx context.Context, u *url.URL, code, clientName string) (desktopServerRecord, error) {
	var empty desktopServerRecord
	if !validDesktopName(strings.TrimSpace(clientName)) || len(code) > 32 || code == "" {
		return empty, ErrDesktopServerInput
	}
	payload, err := json.Marshal(struct {
		Code       string `json:"code"`
		ClientName string `json:"clientName"`
	}{code, strings.TrimSpace(clientName)})
	if err != nil {
		return empty, ErrDesktopServerInput
	}
	ctx, cancel := context.WithTimeout(ctx, 20*time.Second)
	defer cancel()
	target := *u
	target.Path += "/api/v1/connect/redeem"
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, target.String(), bytes.NewReader(payload))
	if err != nil {
		return empty, ErrRemoteUnavailable
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json")
	response, err := s.client.Do(req)
	if err != nil {
		return empty, ErrRemoteUnavailable
	}
	defer response.Body.Close()
	if response.StatusCode == 404 || response.StatusCode == 429 || response.StatusCode == 400 {
		return empty, ErrRemotePairing
	}
	if response.StatusCode != 200 || !gatewayJSON(response.Header) {
		return empty, ErrRemoteUnavailable
	}
	var result struct {
		ClientID   string    `json:"clientId"`
		Credential string    `json:"credential"`
		ExpiresAt  time.Time `json:"expiresAt"`
		Username   string    `json:"username"`
	}
	decoder := json.NewDecoder(io.LimitReader(response.Body, 64<<10))
	if err := decoder.Decode(&result); err != nil {
		return empty, ErrRemoteUnavailable
	}
	if err := decoder.Decode(&struct{}{}); !errors.Is(err, io.EOF) {
		return empty, ErrRemoteUnavailable
	}
	if result.ClientID == "" || len(result.ClientID) > 128 || !validDesktopCredential(result.Credential) || !result.ExpiresAt.After(time.Now()) {
		return empty, ErrRemoteUnavailable
	}
	return desktopServerRecord{ClientID: result.ClientID, Credential: result.Credential, DesktopServer: DesktopServer{ExpiresAt: result.ExpiresAt}}, nil
}

func validDesktopCredential(value string) bool {
	suffix, ok := strings.CutPrefix(value, "boxbox_client_")
	if !ok {
		return false
	}
	decoded, err := base64.RawURLEncoding.DecodeString(suffix)
	return err == nil && len(decoded) == 32 && base64.RawURLEncoding.EncodeToString(decoded) == suffix
}

func (s *DesktopServers) revoke(ctx context.Context, record desktopServerRecord) {
	if record.Credential == "" {
		return
	}
	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	response, err := s.request(ctx, record, http.MethodDelete, "/connect/client", nil, nil, nil)
	if err == nil {
		response.Body.Close()
	}
}
