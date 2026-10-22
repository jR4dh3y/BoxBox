package service

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

const fixedDriveFixture = `Drive(0): KIOXIA-EXCERIA PLUS G3 SSD
  Type: GProxyDrive (GProxyVolumeMonitorUDisks2)
  ids:
   unix-device: '/dev/nvme0n1'
  is_removable=0
  is_media_removable=0
  has_media=1
  can_eject=0
  start_stop_type=shutdown
  Volume(0): 482 GB Volume
    Type: GProxyVolume (GProxyVolumeMonitorUDisks2)
    ids:
     class: 'device'
     unix-device: '/dev/nvme0n1p4'
     uuid: '9646350B4634EE1D'
    can_mount=0
    can_eject=0
    Mount(0): 482 GB Volume -> file:///run/media/radhey/9646350B4634EE1D
      Type: GProxyMount (GProxyVolumeMonitorUDisks2)
      can_unmount=1
      can_eject=0
`

const mountedUSBDriveFixture = fixedDriveFixture + `Drive(1): USB DISK 3.0
  Type: GProxyDrive (GProxyVolumeMonitorUDisks2)
  ids:
   unix-device: '/dev/sdb'
  is_removable=1
  is_media_removable=1
  has_media=1
  can_eject=1
  start_stop_type=external
  Volume(0): Photos
    Type: GProxyVolume (GProxyVolumeMonitorUDisks2)
    ids:
     class: 'device'
     unix-device: '/dev/sdb1'
     uuid: '1234-ABCD'
    can_mount=0
    can_eject=1
    Mount(0): Photos -> file:///run/media/radhey/Photos%20Card
      Type: GProxyMount (GProxyVolumeMonitorUDisks2)
      can_unmount=1
      can_eject=1
`

const unmountedUSBDriveFixture = fixedDriveFixture + `Drive(1): USB DISK 3.0
  Type: GProxyDrive (GProxyVolumeMonitorUDisks2)
  ids:
   unix-device: '/dev/sdb'
  is_removable=1
  is_media_removable=1
  has_media=1
  can_eject=1
  start_stop_type=external
  Volume(0): Photos
    Type: GProxyVolume (GProxyVolumeMonitorUDisks2)
    ids:
     class: 'device'
     unix-device: '/dev/sdb1'
     uuid: '1234-ABCD'
    can_mount=1
    can_eject=0
`

const emptyFixture = fixedDriveFixture

type recordedVolumeCall struct {
	name string
	args []string
}

type fakeVolumeRunner struct {
	listOutputs [][]byte
	listErr     error
	actionErr   error
	listCalls   int
	calls       []recordedVolumeCall
}

func (f *fakeVolumeRunner) Run(_ context.Context, name string, args ...string) ([]byte, error) {
	if name == "gio" && len(args) == 3 && args[0] == "mount" && args[1] == "--list" && args[2] == "--detail" {
		output := f.listOutputs[len(f.listOutputs)-1]
		if f.listCalls < len(f.listOutputs) {
			output = f.listOutputs[f.listCalls]
		}
		f.listCalls++
		if f.listErr != nil {
			return nil, f.listErr
		}
		return output, nil
	}
	f.calls = append(f.calls, recordedVolumeCall{name: name, args: append([]string(nil), args...)})
	if f.actionErr != nil {
		return nil, f.actionErr
	}
	return nil, nil
}

func testDesktopVolumesService(runner volumeCommandRunner, mountPoints []model.MountPoint) *desktopVolumesService {
	return &desktopVolumesService{
		runner:        runner,
		space:         func(string) (volumeSpace, error) { return volumeSpace{total: 64 << 30, free: 16 << 30}, nil },
		mounts:        newMounts(filesystem.NewOsFS(), fixedMounts(mountPoints)),
		refreshTries:  3,
		refreshDelay:  0,
		listTimeout:   time.Second,
		actionTimeout: time.Second,
	}
}

func firstVolume(t *testing.T, response DesktopVolumesResponse) DesktopVolume {
	t.Helper()
	if len(response.Volumes) != 1 {
		t.Fatalf("got %d volumes, want 1", len(response.Volumes))
	}
	return response.Volumes[0]
}

func TestGioMountListFiltersRemovableDrives(t *testing.T) {
	listing := parseGioMountList([]byte(mountedUSBDriveFixture))
	volumes := listing.removableVolumes()
	if len(volumes) != 1 {
		t.Fatalf("removable volumes = %d, want 1", len(volumes))
	}
	found := volumes[0]
	if found.drive.name != "USB DISK 3.0" || found.volume.name != "Photos" {
		t.Fatalf("unexpected removable volume: %q on %q", found.volume.name, found.drive.name)
	}
	if found.volume.ids["unix-device"] != "/dev/sdb1" || found.volume.uuid != "1234-ABCD" {
		t.Fatalf("unexpected identifiers: %#v", found.volume.ids)
	}
	if found.volume.mount == nil || found.volume.mount.uri != "file:///run/media/radhey/Photos%20Card" {
		t.Fatalf("unexpected mount: %#v", found.volume.mount)
	}
	if !found.volume.mount.canUnmount || !found.volume.mount.canEject {
		t.Fatal("mount capabilities were not parsed")
	}
}

func TestDesktopVolumesListDescribesMountedRemovableDrive(t *testing.T) {
	runner := &fakeVolumeRunner{listOutputs: [][]byte{[]byte(mountedUSBDriveFixture)}}
	service := testDesktopVolumesService(runner, nil)

	response, err := service.ListVolumes(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if !response.Available {
		t.Fatal("expected available volume support")
	}
	volume := firstVolume(t, response)
	if volume.Name != "Photos" || volume.DriveName != "USB DISK 3.0" {
		t.Fatalf("unexpected volume: %#v", volume)
	}
	if volume.Device != "/dev/sdb1" || volume.UUID != "1234-ABCD" {
		t.Fatalf("unexpected identity: %#v", volume)
	}
	if volume.MountPath != "/run/media/radhey/Photos Card" {
		t.Fatalf("mount path = %q", volume.MountPath)
	}
	if !volume.Mounted || !volume.CanUnmount || !volume.CanEject || volume.CanMount {
		t.Fatalf("unexpected capabilities: %#v", volume)
	}
	if volume.TotalBytes != 64<<30 || volume.FreeBytes != 16<<30 {
		t.Fatalf("unexpected space: %#v", volume)
	}
	if len(volume.ID) != 32 {
		t.Fatalf("id = %q, want 32 hex characters", volume.ID)
	}
}

func TestDesktopVolumeUnmountUsesReportedURIToKnownVolume(t *testing.T) {
	runner := &fakeVolumeRunner{listOutputs: [][]byte{
		[]byte(mountedUSBDriveFixture),
		[]byte(mountedUSBDriveFixture),
		[]byte(unmountedUSBDriveFixture),
	}}
	service := testDesktopVolumesService(runner, nil)
	initial, err := service.ListVolumes(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	volume := firstVolume(t, initial)

	response, err := service.UnmountVolume(context.Background(), volume.ID)
	if err != nil {
		t.Fatal(err)
	}
	if len(runner.calls) != 1 {
		t.Fatalf("got %d commands, want 1: %#v", len(runner.calls), runner.calls)
	}
	got := runner.calls[0]
	want := []string{"mount", "--unmount", "file:///run/media/radhey/Photos%20Card"}
	if got.name != "gio" || strings.Join(got.args, "\x00") != strings.Join(want, "\x00") {
		t.Fatalf("command = %s %#v, want gio %#v", got.name, got.args, want)
	}
	if firstVolume(t, response).Mounted {
		t.Fatal("response still reports the volume as mounted")
	}
}

func TestDesktopVolumePollsUntilTheUnmountedStateIsVisible(t *testing.T) {
	runner := &fakeVolumeRunner{listOutputs: [][]byte{
		[]byte(mountedUSBDriveFixture),
		[]byte(mountedUSBDriveFixture),
		[]byte(mountedUSBDriveFixture),
		[]byte(unmountedUSBDriveFixture),
	}}
	service := testDesktopVolumesService(runner, nil)
	initial, err := service.ListVolumes(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	volume := firstVolume(t, initial)

	response, err := service.UnmountVolume(context.Background(), volume.ID)
	if err != nil {
		t.Fatal(err)
	}
	if runner.listCalls != 4 {
		t.Fatalf("list calls = %d, want 4 (pre-action, action, stale, settled)", runner.listCalls)
	}
	if firstVolume(t, response).Mounted {
		t.Fatal("stale mount state was returned")
	}
}

func TestDesktopVolumeActionFailsWhenRequestedStateIsNeverObserved(t *testing.T) {
	runner := &fakeVolumeRunner{listOutputs: [][]byte{
		[]byte(mountedUSBDriveFixture),
		[]byte(mountedUSBDriveFixture),
		[]byte(mountedUSBDriveFixture),
		[]byte(mountedUSBDriveFixture),
	}}
	service := testDesktopVolumesService(runner, nil)
	initial, err := service.ListVolumes(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	volume := firstVolume(t, initial)

	response, err := service.UnmountVolume(context.Background(), volume.ID)
	if !errors.Is(err, ErrDesktopVolumeActionUnconfirmed) {
		t.Fatalf("UnmountVolume() error = %v, want unconfirmed action", err)
	}
	if len(response.Volumes) != 0 {
		t.Fatalf("unconfirmed action returned success state: %#v", response)
	}
	var commandErr *DesktopVolumeCommandError
	if !errors.As(err, &commandErr) || !strings.Contains(commandErr.UserMessage(), "did not confirm") {
		t.Fatalf("unconfirmed action has no clear user message: %v", err)
	}
}

func TestDesktopVolumeMountUsesStableUUIDAndRefreshes(t *testing.T) {
	runner := &fakeVolumeRunner{listOutputs: [][]byte{
		[]byte(unmountedUSBDriveFixture),
		[]byte(unmountedUSBDriveFixture),
		[]byte(mountedUSBDriveFixture),
	}}
	service := testDesktopVolumesService(runner, nil)
	initial, err := service.ListVolumes(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	volume := firstVolume(t, initial)
	if volume.CanMount != true {
		t.Fatal("unmounted volume should be mountable")
	}

	response, err := service.MountVolume(context.Background(), volume.ID)
	if err != nil {
		t.Fatal(err)
	}
	if len(runner.calls) != 1 {
		t.Fatalf("got %d commands, want 1", len(runner.calls))
	}
	got := runner.calls[0]
	want := []string{"mount", "--device", "1234-ABCD"}
	if got.name != "gio" || strings.Join(got.args, "\x00") != strings.Join(want, "\x00") {
		t.Fatalf("command = %s %#v, want gio %#v", got.name, got.args, want)
	}
	if !firstVolume(t, response).Mounted {
		t.Fatal("response was not refreshed after mounting")
	}
}

func TestDesktopVolumeEjectRequiresMountedVolume(t *testing.T) {
	runner := &fakeVolumeRunner{listOutputs: [][]byte{[]byte(unmountedUSBDriveFixture)}}
	service := testDesktopVolumesService(runner, nil)
	initial, err := service.ListVolumes(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	volume := firstVolume(t, initial)

	_, err = service.EjectVolume(context.Background(), volume.ID)
	if !errors.Is(err, ErrDesktopVolumeNotMounted) {
		t.Fatalf("error = %v, want ErrDesktopVolumeNotMounted", err)
	}
	if len(runner.calls) != 0 {
		t.Fatal("eject must not run for an unmounted volume")
	}
}

func TestDesktopVolumeEjectTargetsMountedURI(t *testing.T) {
	runner := &fakeVolumeRunner{listOutputs: [][]byte{
		[]byte(mountedUSBDriveFixture),
		[]byte(mountedUSBDriveFixture),
		[]byte(emptyFixture),
	}}
	service := testDesktopVolumesService(runner, nil)
	initial, err := service.ListVolumes(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	volume := firstVolume(t, initial)

	response, err := service.EjectVolume(context.Background(), volume.ID)
	if err != nil {
		t.Fatal(err)
	}
	if len(runner.calls) != 1 {
		t.Fatalf("got %d commands, want 1", len(runner.calls))
	}
	got := runner.calls[0]
	want := []string{"mount", "--eject", "file:///run/media/radhey/Photos%20Card"}
	if got.name != "gio" || strings.Join(got.args, "\x00") != strings.Join(want, "\x00") {
		t.Fatalf("command = %s %#v, want gio %#v", got.name, got.args, want)
	}
	if len(response.Volumes) != 0 {
		t.Fatalf("ejected volume is still listed: %#v", response.Volumes)
	}
}

func TestDesktopVolumeRejectsUnknownIDWithoutRunningACommand(t *testing.T) {
	runner := &fakeVolumeRunner{listOutputs: [][]byte{[]byte(mountedUSBDriveFixture)}}
	service := testDesktopVolumesService(runner, nil)

	for _, action := range []func(context.Context, string) (DesktopVolumesResponse, error){
		service.MountVolume, service.UnmountVolume, service.EjectVolume,
	} {
		_, err := action(context.Background(), "../../dev/sdb1")
		if !errors.Is(err, ErrDesktopVolumeNotFound) {
			t.Fatalf("error = %v, want ErrDesktopVolumeNotFound", err)
		}
	}
	if len(runner.calls) != 0 {
		t.Fatalf("client-supplied ID reached the command runner: %#v", runner.calls)
	}
}

func TestDesktopVolumeActionFailureKeepsTheOSMessage(t *testing.T) {
	runner := &fakeVolumeRunner{
		listOutputs: [][]byte{[]byte(mountedUSBDriveFixture)},
		actionErr:   &volumeCommandError{Name: "gio", Message: "Error unmounting /dev/sdb1: target is busy", Err: errors.New("exit status 2")},
	}
	service := testDesktopVolumesService(runner, nil)
	initial, err := service.ListVolumes(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	volume := firstVolume(t, initial)

	_, err = service.UnmountVolume(context.Background(), volume.ID)
	var commandErr *DesktopVolumeCommandError
	if !errors.As(err, &commandErr) {
		t.Fatalf("error = %T %v, want DesktopVolumeCommandError", err, err)
	}
	if commandErr.Action != DesktopVolumeActionUnmount || commandErr.VolumeName != "Photos" {
		t.Fatalf("unexpected command error context: %#v", commandErr)
	}
	if commandErr.Detail != "Error unmounting /dev/sdb1: target is busy" {
		t.Fatalf("detail = %q", commandErr.Detail)
	}
	if !strings.Contains(commandErr.UserMessage(), "target is busy") {
		t.Fatalf("user message lost the failure reason: %q", commandErr.UserMessage())
	}
}

func TestDesktopVolumesUnavailableWhenGioIsMissing(t *testing.T) {
	runner := &fakeVolumeRunner{
		listOutputs: [][]byte{nil},
		listErr:     &volumeCommandError{Name: "gio", Message: "", Err: exec.ErrNotFound},
	}
	service := testDesktopVolumesService(runner, nil)

	response, err := service.ListVolumes(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if response.Available {
		t.Fatal("missing gio must report unavailable")
	}
	if !strings.Contains(response.Reason, "not installed") {
		t.Fatalf("reason = %q", response.Reason)
	}
	if _, err := service.MountVolume(context.Background(), "anything"); !errors.Is(err, ErrDesktopVolumesUnavailable) {
		t.Fatalf("mount error = %v, want ErrDesktopVolumesUnavailable", err)
	}
}

func TestDesktopVolumeIDStableWhenDriveOrderChanges(t *testing.T) {
	first := parseGioMountList([]byte(mountedUSBDriveFixture)).removableVolumes()
	reordered := strings.Replace(mountedUSBDriveFixture, "Drive(1): USB DISK 3.0", "Drive(7): USB DISK 3.0", 1)
	second := parseGioMountList([]byte(reordered)).removableVolumes()
	if len(first) != 1 || len(second) != 1 {
		t.Fatal("fixture must contain one removable volume")
	}
	if desktopVolumeID(first[0]) != desktopVolumeID(second[0]) {
		t.Fatal("volume ID changed when only the drive index changed")
	}
}

func TestDesktopVolumeBrowsePathIsResolvedThroughMounts(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "run", "media", "radhey", "Photos Card")
	if err := os.MkdirAll(target, 0o755); err != nil {
		t.Fatal(err)
	}
	service := &desktopVolumesService{
		mounts: newMounts(filesystem.NewOsFS(), fixedMounts([]model.MountPoint{{Name: "root", Path: root}})),
	}
	if got := service.browsePath(target); got != "root/run/media/radhey/Photos Card" {
		t.Fatalf("browse path = %q", got)
	}
	nestedRoot := filepath.Join(root, "tmp")
	nestedTarget := filepath.Join(nestedRoot, "volume")
	if err := os.MkdirAll(nestedTarget, 0o755); err != nil {
		t.Fatal(err)
	}
	service.mounts = newMounts(filesystem.NewOsFS(), fixedMounts([]model.MountPoint{
		{Name: "root", Path: root},
		{Name: "nested", Path: nestedRoot},
	}))
	if got := service.browsePath(nestedTarget); got != "nested/volume" {
		t.Fatalf("nested browse path = %q, want the most specific mount", got)
	}
	outside := filepath.Join(t.TempDir(), "not-mounted")
	if err := os.MkdirAll(outside, 0o755); err != nil {
		t.Fatal(err)
	}
	if got := service.browsePath(outside); got != "" {
		t.Fatalf("outside path produced %q", got)
	}
}

func TestDesktopVolumeReadOnlyFlagComesFromMountStats(t *testing.T) {
	runner := &fakeVolumeRunner{listOutputs: [][]byte{[]byte(mountedUSBDriveFixture)}}
	service := testDesktopVolumesService(runner, nil)
	service.space = func(string) (volumeSpace, error) {
		return volumeSpace{total: 1024, free: 0, readOnly: true}, nil
	}
	response, err := service.ListVolumes(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	volume := firstVolume(t, response)
	if !volume.ReadOnly || volume.TotalBytes != 1024 || volume.FreeBytes != 0 {
		t.Fatalf("unexpected read-only state: %#v", volume)
	}
}
