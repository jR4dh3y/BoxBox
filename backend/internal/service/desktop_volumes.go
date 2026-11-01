package service

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"net/url"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

const (
	desktopVolumesListTimeout   = 5 * time.Second
	desktopVolumeActionTimeout  = 45 * time.Second
	desktopVolumesRefreshTries  = 8
	desktopVolumesRefreshDelay  = 100 * time.Millisecond
	desktopVolumeDetailMaxRunes = 240
)

// DesktopVolumeAction is a removable-volume operation offered by the desktop app.
type DesktopVolumeAction string

const (
	DesktopVolumeActionMount   DesktopVolumeAction = "mount"
	DesktopVolumeActionUnmount DesktopVolumeAction = "unmount"
	DesktopVolumeActionEject   DesktopVolumeAction = "eject"
)

// DesktopVolume is one removable volume reported by GIO. The ID is stable while
// the volume's UUID, device or drive identity stays the same; clients must send
// it back instead of a device path.
type DesktopVolume struct {
	ID         string `json:"id"`
	Name       string `json:"name"`
	DriveName  string `json:"driveName"`
	Device     string `json:"device,omitempty"`
	UUID       string `json:"uuid,omitempty"`
	MountURI   string `json:"mountUri,omitempty"`
	MountPath  string `json:"mountPath,omitempty"`
	BrowsePath string `json:"browsePath,omitempty"`
	Mounted    bool   `json:"mounted"`
	ReadOnly   bool   `json:"readOnly"`
	TotalBytes uint64 `json:"totalBytes,omitempty"`
	FreeBytes  uint64 `json:"freeBytes,omitempty"`
	CanMount   bool   `json:"canMount"`
	CanUnmount bool   `json:"canUnmount"`
	CanEject   bool   `json:"canEject"`
}

// DesktopVolumesResponse always describes the current state. Available is false
// when the user session has no GIO/UDisks volume monitor.
type DesktopVolumesResponse struct {
	Available bool            `json:"available"`
	Reason    string          `json:"reason,omitempty"`
	Volumes   []DesktopVolume `json:"volumes"`
}

var (
	ErrDesktopVolumesUnavailable      = errors.New("removable volume support is unavailable")
	ErrDesktopVolumeNotFound          = errors.New("removable volume not found")
	ErrDesktopVolumeNotMounted        = errors.New("removable volume is not mounted")
	ErrDesktopVolumeActionUnconfirmed = errors.New("removable volume action was not confirmed")
)

// DesktopVolumeCommandError reports a failed GIO operation. Detail comes from
// the OS service so the desktop UI can explain why the drive did not change.
type DesktopVolumeCommandError struct {
	Action     DesktopVolumeAction
	VolumeName string
	Detail     string
	Err        error
}

func (e *DesktopVolumeCommandError) Error() string {
	if e.Detail == "" {
		return fmt.Sprintf("could not %s %s", e.Action, e.VolumeName)
	}
	return fmt.Sprintf("could not %s %s: %s", e.Action, e.VolumeName, e.Detail)
}

func (e *DesktopVolumeCommandError) Unwrap() error { return e.Err }

// UserMessage is safe to show in the desktop UI.
func (e *DesktopVolumeCommandError) UserMessage() string {
	message := fmt.Sprintf("Could not %s %s", e.Action, e.VolumeName)
	if e.Detail != "" {
		message += ": " + e.Detail
	}
	return message
}

// DesktopVolumesService lists removable drives and performs validated actions.
type DesktopVolumesService interface {
	ListVolumes(ctx context.Context) (DesktopVolumesResponse, error)
	MountVolume(ctx context.Context, volumeID string) (DesktopVolumesResponse, error)
	UnmountVolume(ctx context.Context, volumeID string) (DesktopVolumesResponse, error)
	EjectVolume(ctx context.Context, volumeID string) (DesktopVolumesResponse, error)
}

type volumeCommandRunner interface {
	Run(ctx context.Context, name string, args ...string) ([]byte, error)
}

type execVolumeCommandRunner struct{}

func (execVolumeCommandRunner) Run(ctx context.Context, name string, args ...string) ([]byte, error) {
	command := exec.CommandContext(ctx, name, args...)
	var stderr bytes.Buffer
	command.Stderr = &stderr
	output, err := command.Output()
	if err != nil {
		if contextErr := ctx.Err(); contextErr != nil {
			return nil, &volumeCommandError{Name: name, Err: contextErr}
		}
		detail := strings.TrimSpace(stderr.String())
		if detail == "" {
			detail = err.Error()
		}
		return nil, &volumeCommandError{Name: name, Message: detail, Err: err}
	}
	return output, nil
}

type volumeCommandError struct {
	Name    string
	Message string
	Err     error
}

func (e *volumeCommandError) Error() string {
	return fmt.Sprintf("%s failed: %s", e.Name, e.Message)
}

func (e *volumeCommandError) Unwrap() error { return e.Err }

type volumeSpace struct {
	total    uint64
	free     uint64
	readOnly bool
}

type volumeSpaceReader func(path string) (volumeSpace, error)

type desktopVolumesService struct {
	runner        volumeCommandRunner
	space         volumeSpaceReader
	mounts        *mounts
	refreshTries  int
	refreshDelay  time.Duration
	listTimeout   time.Duration
	actionTimeout time.Duration
}

// NewDesktopVolumesService lists removable drives for the configured mounts.
// A volume's browse path is accepted only after mounts.resolve confirms that it
// maps back to the same real mount point.
func NewDesktopVolumesService(mountPoints []model.MountPoint) DesktopVolumesService {
	return &desktopVolumesService{
		runner:        execVolumeCommandRunner{},
		space:         readVolumeSpace,
		mounts:        newMounts(filesystem.NewOsFS(), fixedMounts(mountPoints)),
		refreshTries:  desktopVolumesRefreshTries,
		refreshDelay:  desktopVolumesRefreshDelay,
		listTimeout:   desktopVolumesListTimeout,
		actionTimeout: desktopVolumeActionTimeout,
	}
}

func (s *desktopVolumesService) ListVolumes(ctx context.Context) (DesktopVolumesResponse, error) {
	snapshot, err := s.snapshot(ctx)
	if err != nil {
		if errors.Is(err, ErrDesktopVolumesUnavailable) {
			return DesktopVolumesResponse{
				Available: false,
				Reason:    unavailableDetail(err),
				Volumes:   []DesktopVolume{},
			}, nil
		}
		return DesktopVolumesResponse{}, err
	}
	return snapshot.response, nil
}

func (s *desktopVolumesService) MountVolume(ctx context.Context, volumeID string) (DesktopVolumesResponse, error) {
	return s.perform(ctx, volumeID, DesktopVolumeActionMount)
}

func (s *desktopVolumesService) UnmountVolume(ctx context.Context, volumeID string) (DesktopVolumesResponse, error) {
	return s.perform(ctx, volumeID, DesktopVolumeActionUnmount)
}

func (s *desktopVolumesService) EjectVolume(ctx context.Context, volumeID string) (DesktopVolumesResponse, error) {
	return s.perform(ctx, volumeID, DesktopVolumeActionEject)
}

func (s *desktopVolumesService) perform(ctx context.Context, volumeID string, action DesktopVolumeAction) (DesktopVolumesResponse, error) {
	before, err := s.snapshot(ctx)
	if err != nil {
		return DesktopVolumesResponse{}, err
	}
	target, ok := before.volumes[volumeID]
	if !ok {
		return DesktopVolumesResponse{}, ErrDesktopVolumeNotFound
	}

	args, err := actionArguments(target, action)
	if err != nil {
		return DesktopVolumesResponse{}, err
	}
	if args == nil {
		return before.response, nil
	}

	actionCtx, cancel := context.WithTimeout(context.WithoutCancel(ctx), s.actionTimeout)
	defer cancel()
	if _, err := s.runner.Run(actionCtx, "gio", args...); err != nil {
		return DesktopVolumesResponse{}, &DesktopVolumeCommandError{
			Action:     action,
			VolumeName: target.name,
			Detail:     commandDetail(err),
			Err:        err,
		}
	}
	return s.waitForAction(ctx, action, volumeID, target.name)
}

// actionArguments returns nil when the requested state already holds.
func actionArguments(target desktopVolumeTarget, action DesktopVolumeAction) ([]string, error) {
	switch action {
	case DesktopVolumeActionMount:
		if target.mounted {
			return nil, nil
		}
		identifier := target.mountIdentifier()
		if identifier == "" {
			return nil, &DesktopVolumeCommandError{
				Action:     action,
				VolumeName: target.name,
				Detail:     "This volume has no stable identifier to mount.",
			}
		}
		return []string{"mount", "--device", identifier}, nil
	case DesktopVolumeActionUnmount:
		if !target.mounted {
			return nil, nil
		}
		uri, err := removableFileURI(target.mountURI, action, target.name)
		if err != nil {
			return nil, err
		}
		return []string{"mount", "--unmount", uri}, nil
	case DesktopVolumeActionEject:
		if !target.mounted {
			return nil, ErrDesktopVolumeNotMounted
		}
		uri, err := removableFileURI(target.mountURI, action, target.name)
		if err != nil {
			return nil, err
		}
		return []string{"mount", "--eject", uri}, nil
	default:
		return nil, fmt.Errorf("unsupported volume action %q", action)
	}
}

func removableFileURI(uri string, action DesktopVolumeAction, name string) (string, error) {
	parsed, err := url.Parse(uri)
	if err != nil || parsed.Scheme != "file" || parsed.Path == "" {
		return "", &DesktopVolumeCommandError{
			Action:     action,
			VolumeName: name,
			Detail:     "The mount location reported by the system is not a local file URI.",
		}
	}
	return uri, nil
}

func (s *desktopVolumesService) waitForAction(ctx context.Context, action DesktopVolumeAction, volumeID, volumeName string) (DesktopVolumesResponse, error) {
	var last DesktopVolumesResponse
	for attempt := 0; attempt < s.refreshTries; attempt++ {
		snapshot, err := s.snapshot(ctx)
		if err != nil {
			return DesktopVolumesResponse{}, err
		}
		last = snapshot.response
		if actionSettled(snapshot.volumes, action, volumeID) {
			return last, nil
		}
		if attempt == s.refreshTries-1 {
			break
		}
		timer := time.NewTimer(s.refreshDelay)
		select {
		case <-ctx.Done():
			timer.Stop()
			return DesktopVolumesResponse{}, &DesktopVolumeCommandError{
				Action:     action,
				VolumeName: volumeName,
				Detail:     "The operation ended before the new volume state could be confirmed.",
				Err:        fmt.Errorf("%w: %w", ErrDesktopVolumeActionUnconfirmed, ctx.Err()),
			}
		case <-timer.C:
		}
	}
	return DesktopVolumesResponse{}, &DesktopVolumeCommandError{
		Action:     action,
		VolumeName: volumeName,
		Detail:     "The system did not confirm that the volume changed state.",
		Err:        ErrDesktopVolumeActionUnconfirmed,
	}
}

func actionSettled(volumes map[string]desktopVolumeTarget, action DesktopVolumeAction, volumeID string) bool {
	target, exists := volumes[volumeID]
	switch action {
	case DesktopVolumeActionMount:
		return exists && target.mounted
	case DesktopVolumeActionUnmount, DesktopVolumeActionEject:
		return !exists || !target.mounted
	default:
		return false
	}
}

type desktopVolumeSnapshot struct {
	response DesktopVolumesResponse
	volumes  map[string]desktopVolumeTarget
}

type desktopVolumeTarget struct {
	name          string
	device        string
	uuid          string
	mountURI      string
	mounted       bool
	mountCanEject bool
	canEject      bool
}

func (t desktopVolumeTarget) mountIdentifier() string {
	if t.uuid != "" {
		return t.uuid
	}
	return t.device
}

func (s *desktopVolumesService) snapshot(ctx context.Context) (desktopVolumeSnapshot, error) {
	listCtx, cancel := context.WithTimeout(ctx, s.listTimeout)
	defer cancel()
	output, err := s.runner.Run(listCtx, "gio", "mount", "--list", "--detail")
	if err != nil {
		return desktopVolumeSnapshot{}, fmt.Errorf("%w: %s", ErrDesktopVolumesUnavailable, commandDetail(err))
	}

	listing := parseGioMountList(output)
	snapshot := desktopVolumeSnapshot{
		response: DesktopVolumesResponse{Available: true, Volumes: []DesktopVolume{}},
		volumes:  make(map[string]desktopVolumeTarget),
	}
	for _, found := range listing.removableVolumes() {
		volume, target := s.describeVolume(found)
		if _, duplicate := snapshot.volumes[volume.ID]; duplicate {
			continue
		}
		snapshot.volumes[volume.ID] = target
		snapshot.response.Volumes = append(snapshot.response.Volumes, volume)
	}
	sort.Slice(snapshot.response.Volumes, func(i, j int) bool {
		left, right := snapshot.response.Volumes[i], snapshot.response.Volumes[j]
		if left.DriveName != right.DriveName {
			return left.DriveName < right.DriveName
		}
		if left.Name != right.Name {
			return left.Name < right.Name
		}
		return left.Device < right.Device
	})
	return snapshot, nil
}

func (s *desktopVolumesService) describeVolume(found gioVolumeRef) (DesktopVolume, desktopVolumeTarget) {
	volume := DesktopVolume{
		ID:        desktopVolumeID(found),
		Name:      found.volume.name,
		DriveName: found.drive.name,
		Device:    found.volume.ids["unix-device"],
		UUID:      found.volume.uuid,
		Mounted:   found.volume.mount != nil,
	}
	if volume.Name == "" {
		volume.Name = "Removable volume"
	}
	if found.volume.mount != nil {
		volume.MountURI = found.volume.mount.uri
		volume.MountPath = fileURIPath(found.volume.mount.uri)
		volume.BrowsePath = s.browsePath(volume.MountPath)
		if space, err := s.space(volume.MountPath); err == nil {
			volume.TotalBytes = space.total
			volume.FreeBytes = space.free
			volume.ReadOnly = space.readOnly
		}
	}

	target := desktopVolumeTarget{
		name:          volume.Name,
		device:        volume.Device,
		uuid:          volume.UUID,
		mountURI:      volume.MountURI,
		mounted:       volume.Mounted,
		mountCanEject: found.volume.mount != nil && found.volume.mount.canEject,
		canEject:      found.drive.canEject || found.volume.canEject,
	}
	volume.CanMount = !volume.Mounted && found.volume.canMount && target.mountIdentifier() != ""
	volume.CanUnmount = volume.Mounted && volume.MountURI != "" && found.volume.mount.canUnmount
	volume.CanEject = volume.Mounted && volume.MountURI != "" && (target.mountCanEject || target.canEject)
	return volume, target
}

func (s *desktopVolumesService) browsePath(realPath string) string {
	if realPath == "" || s.mounts == nil {
		return ""
	}
	canonical := realPath
	if evaluated, err := filepath.EvalSymlinks(realPath); err == nil {
		canonical = evaluated
	}
	candidates := append([]model.MountPoint(nil), s.mounts.list()...)
	sort.SliceStable(candidates, func(i, j int) bool {
		return len(candidates[i].Path) > len(candidates[j].Path)
	})
	for _, mount := range candidates {
		relative, err := filepath.Rel(mount.Path, canonical)
		if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
			continue
		}
		virtualPath := mount.Name
		if relative != "." {
			virtualPath += "/" + filepath.ToSlash(relative)
		}
		_, resolved, err := s.mounts.resolve(virtualPath, readExisting)
		if err == nil && resolved == canonical {
			return virtualPath
		}
	}
	return ""
}

func desktopVolumeID(found gioVolumeRef) string {
	identity := found.volume.uuid
	if identity == "" {
		identity = found.volume.ids["unix-device"]
	}
	if identity == "" {
		identity = found.drive.ids["unix-device"] + "/" + found.volume.name
	}
	sum := sha256.Sum256([]byte(identity))
	return hex.EncodeToString(sum[:16])
}

func fileURIPath(uri string) string {
	parsed, err := url.Parse(uri)
	if err != nil || parsed.Scheme != "file" || parsed.Path == "" {
		return ""
	}
	return filepath.Clean(filepath.FromSlash(parsed.Path))
}

func commandDetail(err error) string {
	detail := ""
	switch {
	case errors.Is(err, exec.ErrNotFound):
		detail = "The gio command is not installed."
	case errors.Is(err, context.DeadlineExceeded):
		detail = "The system volume service did not respond."
	default:
		var commandErr *volumeCommandError
		if errors.As(err, &commandErr) {
			detail = commandErr.Message
		} else {
			detail = "The system volume service could not complete the request."
		}
	}
	return truncateDetail(detail)
}

func unavailableDetail(err error) string {
	message := err.Error()
	if _, detail, ok := strings.Cut(message, ": "); ok {
		return truncateDetail(detail)
	}
	return "Removable drives need the GIO volume monitor and UDisks."
}

func truncateDetail(detail string) string {
	detail = strings.Join(strings.Fields(detail), " ")
	if detail == "" {
		return "The system volume service could not complete the request."
	}
	runes := []rune(detail)
	if len(runes) > desktopVolumeDetailMaxRunes {
		return string(runes[:desktopVolumeDetailMaxRunes]) + "…"
	}
	return detail
}
