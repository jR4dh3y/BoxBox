package handler

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
)

type stubDesktopVolumesService struct {
	response service.DesktopVolumesResponse
	err      error
	volumeID string
	action   service.DesktopVolumeAction
}

func (s *stubDesktopVolumesService) ListVolumes(context.Context) (service.DesktopVolumesResponse, error) {
	return s.response, s.err
}

func (s *stubDesktopVolumesService) MountVolume(_ context.Context, volumeID string) (service.DesktopVolumesResponse, error) {
	s.volumeID = volumeID
	s.action = service.DesktopVolumeActionMount
	return s.response, s.err
}

func (s *stubDesktopVolumesService) UnmountVolume(_ context.Context, volumeID string) (service.DesktopVolumesResponse, error) {
	s.volumeID = volumeID
	s.action = service.DesktopVolumeActionUnmount
	return s.response, s.err
}

func (s *stubDesktopVolumesService) EjectVolume(_ context.Context, volumeID string) (service.DesktopVolumesResponse, error) {
	s.volumeID = volumeID
	s.action = service.DesktopVolumeActionEject
	return s.response, s.err
}

func desktopVolumesRouter(stub *stubDesktopVolumesService) http.Handler {
	router := chi.NewRouter()
	NewDesktopVolumesHandler(stub).RegisterRoutes(router)
	return router
}

func TestDesktopVolumesListReturnsSnapshot(t *testing.T) {
	stub := &stubDesktopVolumesService{response: service.DesktopVolumesResponse{
		Available: true,
		Volumes: []service.DesktopVolume{{
			ID: "abc", Name: "Photos", DriveName: "USB DISK", Mounted: true,
		}},
	}}
	recorder := httptest.NewRecorder()
	desktopVolumesRouter(stub).ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, "/volumes", nil))

	if recorder.Code != http.StatusOK {
		t.Fatalf("status = %d", recorder.Code)
	}
	if recorder.Header().Get("Cache-Control") != "no-store" {
		t.Fatal("volume snapshots must not be cached")
	}
	var response service.DesktopVolumesResponse
	if err := json.NewDecoder(recorder.Body).Decode(&response); err != nil {
		t.Fatal(err)
	}
	if !response.Available || len(response.Volumes) != 1 || response.Volumes[0].ID != "abc" {
		t.Fatalf("unexpected response: %#v", response)
	}
}

func TestDesktopVolumesActionPassesPathIDAndReturnsFreshList(t *testing.T) {
	for _, tc := range []struct {
		path   string
		action service.DesktopVolumeAction
	}{
		{"/volumes/abc%20123/mount", service.DesktopVolumeActionMount},
		{"/volumes/abc%20123/unmount", service.DesktopVolumeActionUnmount},
		{"/volumes/abc%20123/eject", service.DesktopVolumeActionEject},
	} {
		t.Run(string(tc.action), func(t *testing.T) {
			stub := &stubDesktopVolumesService{response: service.DesktopVolumesResponse{
				Available: true,
				Volumes:   []service.DesktopVolume{{ID: "abc 123", Mounted: tc.action == service.DesktopVolumeActionMount}},
			}}
			recorder := httptest.NewRecorder()
			desktopVolumesRouter(stub).ServeHTTP(recorder, httptest.NewRequest(http.MethodPost, tc.path, nil))
			if recorder.Code != http.StatusOK {
				t.Fatalf("status = %d", recorder.Code)
			}
			if stub.volumeID != "abc 123" || stub.action != tc.action {
				t.Fatalf("service got %q %q", stub.volumeID, stub.action)
			}
			var response service.DesktopVolumesResponse
			if err := json.NewDecoder(recorder.Body).Decode(&response); err != nil {
				t.Fatal(err)
			}
			if len(response.Volumes) != 1 {
				t.Fatalf("unexpected response: %#v", response)
			}
		})
	}
}

func TestDesktopVolumesErrorsMapToUserFacingStatuses(t *testing.T) {
	for _, tc := range []struct {
		name        string
		err         error
		wantStatus  int
		wantCode    string
		wantMessage string
	}{
		{
			name:        "not found",
			err:         service.ErrDesktopVolumeNotFound,
			wantStatus:  http.StatusNotFound,
			wantCode:    model.ErrCodeNotFound,
			wantMessage: "no longer connected",
		},
		{
			name:        "unavailable",
			err:         service.ErrDesktopVolumesUnavailable,
			wantStatus:  http.StatusServiceUnavailable,
			wantCode:    "VOLUMES_UNAVAILABLE",
			wantMessage: "unavailable",
		},
		{
			name:        "not mounted",
			err:         service.ErrDesktopVolumeNotMounted,
			wantStatus:  http.StatusConflict,
			wantCode:    model.ErrCodeConflict,
			wantMessage: "Unmount",
		},
		{
			name: "command failed",
			err: &service.DesktopVolumeCommandError{
				Action: service.DesktopVolumeActionUnmount, VolumeName: "Photos", Detail: "target is busy",
			},
			wantStatus:  http.StatusConflict,
			wantCode:    model.ErrCodeConflict,
			wantMessage: "target is busy",
		},
		{
			name:        "unexpected",
			err:         errors.New("boom"),
			wantStatus:  http.StatusInternalServerError,
			wantCode:    model.ErrCodeInternalError,
			wantMessage: "Could not update",
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			stub := &stubDesktopVolumesService{err: tc.err}
			recorder := httptest.NewRecorder()
			desktopVolumesRouter(stub).ServeHTTP(recorder, httptest.NewRequest(http.MethodPost, "/volumes/abc/mount", nil))
			if recorder.Code != tc.wantStatus {
				t.Fatalf("status = %d, want %d", recorder.Code, tc.wantStatus)
			}
			var body model.ErrorResponse
			if err := json.NewDecoder(recorder.Body).Decode(&body); err != nil {
				t.Fatal(err)
			}
			if body.Code != tc.wantCode || !strings.Contains(body.Error, tc.wantMessage) {
				t.Fatalf("body = %#v, want code %q containing %q", body, tc.wantCode, tc.wantMessage)
			}
		})
	}
}

func TestDesktopVolumesUnavailableJSONKeepsReasonAndEmptyArray(t *testing.T) {
	t.Setenv("PATH", t.TempDir())
	router := chi.NewRouter()
	NewDesktopVolumesHandler(service.NewDesktopVolumesService(nil)).RegisterRoutes(router)

	recorder := httptest.NewRecorder()
	router.ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, "/volumes", nil))
	if recorder.Code != http.StatusOK {
		t.Fatalf("status = %d", recorder.Code)
	}

	var body struct {
		Available bool            `json:"available"`
		Reason    string          `json:"reason"`
		Volumes   json.RawMessage `json:"volumes"`
	}
	if err := json.NewDecoder(recorder.Body).Decode(&body); err != nil {
		t.Fatal(err)
	}
	if body.Available {
		t.Fatal("unavailable service was reported as available")
	}
	if !strings.Contains(body.Reason, "gio command") {
		t.Fatalf("reason = %q, want the real gio failure", body.Reason)
	}
	if string(body.Volumes) != "[]" {
		t.Fatalf("volumes = %s, want an empty array", body.Volumes)
	}
}

func TestDesktopVolumesListServiceFailureDoesNotLeakDetails(t *testing.T) {
	stub := &stubDesktopVolumesService{err: errors.New("dbus secret detail")}
	recorder := httptest.NewRecorder()
	desktopVolumesRouter(stub).ServeHTTP(recorder, httptest.NewRequest(http.MethodGet, "/volumes", nil))
	if recorder.Code != http.StatusInternalServerError {
		t.Fatalf("status = %d", recorder.Code)
	}
	if strings.Contains(recorder.Body.String(), "dbus secret detail") {
		t.Fatal("internal failure detail leaked to the client")
	}
}
