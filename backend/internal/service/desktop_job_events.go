package service

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"strings"
	"time"
)

// RemoteJobEvent keeps each deployment's identity outside its untrusted payload.
type RemoteJobEvent struct {
	ServerID string          `json:"serverId"`
	Kind     string          `json:"kind"`
	Data     json.RawMessage `json:"data,omitempty"`
}

// StreamJobs reconnects each upstream independently of the browser connection.
func (s *DesktopServers) StreamJobs(ctx context.Context, id string, emit func(RemoteJobEvent) bool) {
	delay := time.Second
	send := func(kind string, data json.RawMessage) bool {
		return emit(RemoteJobEvent{ServerID: id, Kind: kind, Data: data})
	}
	for ctx.Err() == nil {
		response, err := s.openJobEvents(ctx, id)
		if err == nil && (response.StatusCode == 404 || response.StatusCode == 405) {
			response.Body.Close()
			// Older servers discover once; the renderer recovers only known active jobs.
			snapshot, listErr := s.Forward(ctx, id, http.MethodGet, "/jobs", nil, nil, nil)
			if listErr == nil {
				data, readErr := io.ReadAll(io.LimitReader(snapshot.Body, (4<<20)+1))
				snapshot.Body.Close()
				if snapshot.StatusCode == 200 && readErr == nil && len(data) <= 4<<20 && json.Valid(data) {
					if !send("snapshot", data) {
						return
					}
				}
			}
			send("unavailable", nil)
			return
		}
		if err == nil {
			if response.StatusCode == 200 && strings.HasPrefix(response.Header.Get("Content-Type"), "text/event-stream") {
				readRemoteJobEvents(response.Body, func(kind string, data json.RawMessage) bool {
					delay = time.Second
					return send(kind, data)
				})
			}
			response.Body.Close()
		}
		if ctx.Err() != nil {
			return
		}
		if errors.Is(err, ErrRemoteAuthRequired) || errors.Is(err, ErrDesktopServerNotFound) {
			send("blocked", nil)
			return
		}
		if !send("unavailable", nil) {
			return
		}
		timer := time.NewTimer(delay)
		select {
		case <-ctx.Done():
			timer.Stop()
			return
		case <-timer.C:
		}
		delay = min(delay*2, 30*time.Second)
	}
}

func readRemoteJobEvents(input io.Reader, emit func(string, json.RawMessage) bool) {
	scanner := bufio.NewScanner(input)
	scanner.Buffer(make([]byte, 4096), 4<<20)
	var kind string
	var data strings.Builder
	for scanner.Scan() {
		line := scanner.Text()
		if line == "" {
			if kind == "snapshot" || kind == "job" {
				payload := json.RawMessage(data.String())
				if !json.Valid(payload) || !emit(kind, payload) {
					return
				}
			}
			kind = ""
			data.Reset()
		} else if strings.HasPrefix(line, "event:") {
			kind = strings.TrimSpace(strings.TrimPrefix(line, "event:"))
		} else if strings.HasPrefix(line, "data:") {
			if data.Len() > 0 {
				data.WriteByte('\n')
			}
			data.WriteString(strings.TrimPrefix(strings.TrimPrefix(line, "data:"), " "))
			if data.Len() > 4<<20 {
				return
			}
		}
	}
}

// The fixed events route may return HTML 404s on older deployments.
func (s *DesktopServers) openJobEvents(ctx context.Context, id string) (*http.Response, error) {
	record, err := s.record(id)
	if err != nil {
		return nil, err
	}
	if record.Credential == "" || !record.ExpiresAt.After(time.Now()) {
		return nil, ErrRemoteAuthRequired
	}
	return s.request(ctx, record, http.MethodGet, "/jobs/events", nil, nil, nil)
}
