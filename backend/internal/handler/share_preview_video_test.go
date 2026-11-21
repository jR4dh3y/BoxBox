package handler

import (
	"bytes"
	"image/jpeg"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"github.com/jR4dh3y/BoxBox/backend/internal/static"
)

func missingVideoThumbnailPrerequisite(t *testing.T, message string) {
	t.Helper()
	if os.Getenv("BOXBOX_REQUIRE_VIDEO_THUMBNAIL_TEST") == "1" {
		t.Fatal(message)
	}
	t.Skip(message)
}

func TestSharePreviewVideo(t *testing.T) {
	if runtime.GOOS != "linux" {
		missingVideoThumbnailPrerequisite(t, "video previews require Linux")
	}
	ffmpeg, err := exec.LookPath("ffmpeg")
	if err != nil {
		missingVideoThumbnailPrerequisite(t, "ffmpeg unavailable")
	}
	details, err := exec.Command(ffmpeg, "-hide_banner", "-h", "protocol=fd").CombinedOutput()
	if err != nil || !bytes.Contains(details, []byte("fd AVOptions:")) {
		missingVideoThumbnailPrerequisite(t, "ffmpeg does not support seekable descriptors")
	}
	prlimit, err := exec.LookPath("prlimit")
	if err != nil {
		missingVideoThumbnailPrerequisite(t, "prlimit unavailable")
	}

	mount := t.TempDir()
	name := "short clip.MP4"
	path := filepath.Join(mount, name)
	// A short clip forces the renderer to try the opening frame. The default MP4 muxer writes moov after mdat.
	cmd := exec.Command(ffmpeg, "-nostdin", "-v", "error", "-f", "lavfi", "-i", "color=red:s=1600x900:r=10",
		"-t", "0.2", "-c:v", "mpeg4", "-threads", "1", "-f", "mp4", path)
	if output, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("create test video: %v: %s", err, output)
	}

	tools := t.TempDir()
	ffmpegLink := filepath.Join(tools, "ffmpeg")
	for link, binary := range map[string]string{ffmpegLink: ffmpeg, filepath.Join(tools, "prlimit"): prlimit} {
		if err := os.Symlink(binary, link); err != nil {
			t.Fatal(err)
		}
	}
	t.Setenv("PATH", tools)

	shares := service.NewShareService(filesystem.NewOsFS(), service.ShareServiceConfig{
		DataDir: t.TempDir(),
		Mounts:  func() []model.MountPoint { return []model.MountPoint{{Name: "media", Path: mount}} },
	})
	owner := createShareTestRouter(NewShareHandler(shares, nil, 1))
	share := createShareViaAPI(t, owner, "media/"+name, model.SharePermissions{View: true}, nil)
	f := newPreviewFixture(t, shares, "https://files.example.com")
	meta := f.page(t, share.Token, nil)
	if meta.Image == nil || meta.Card != static.TwitterCardLargeImage ||
		!strings.HasPrefix(meta.Image.URL, "https://files.example.com/api/v1/share/"+url.PathEscape(share.Token)+"/thumbnail?v=") {
		t.Fatalf("video meta = %+v", meta)
	}

	first := thumbnailRequest(f, share.Token, "")
	etag := first.Header().Get("ETag")
	if first.Code != http.StatusOK || first.Header().Get("Content-Type") != "image/jpeg" ||
		first.Header().Get("Cache-Control") != "no-cache" || etag == "" {
		t.Fatalf("video thumbnail = %d %v", first.Code, first.Header())
	}
	img, err := jpeg.Decode(bytes.NewReader(first.Body.Bytes()))
	if err != nil {
		t.Fatal(err)
	}
	if img.Bounds().Dx() != 1200 || img.Bounds().Dy() != 675 {
		t.Fatalf("JPEG dimensions = %v, want 1200x675", img.Bounds().Size())
	}
	r, g, b, _ := img.At(600, 300).RGBA()
	if r < 0xe000 || g > 0x1000 || b > 0x1000 {
		t.Fatalf("JPEG did not show the red video frame: RGB = %x %x %x", r, g, b)
	}

	if err := os.Remove(ffmpegLink); err != nil {
		t.Fatal(err)
	}
	warm := thumbnailRequest(f, share.Token, "")
	if warm.Code != http.StatusOK || warm.Header().Get("ETag") != etag || !bytes.Equal(warm.Body.Bytes(), first.Body.Bytes()) {
		t.Fatalf("cached video thumbnail = %d %v", warm.Code, warm.Header())
	}
	conditional := thumbnailRequest(f, share.Token, etag)
	if conditional.Code != http.StatusNotModified || conditional.Body.Len() != 0 {
		t.Fatalf("conditional video thumbnail = %d (%d bytes)", conditional.Code, conditional.Body.Len())
	}

	withoutTools := newPreviewFixture(t, shares, "https://files.example.com")
	meta = withoutTools.page(t, share.Token, nil)
	if meta.Image == nil || meta.Image.URL != "https://files.example.com/og-default.png" {
		t.Fatalf("video meta with FFmpeg missing = %+v", meta)
	}
	fallback := thumbnailRequest(withoutTools, share.Token, etag)
	if fallback.Code != http.StatusOK || fallback.Header().Get("Content-Type") != "image/png" {
		t.Fatalf("video thumbnail with FFmpeg missing = %d %v", fallback.Code, fallback.Header())
	}

	if err := shares.Revoke("owner", share.ID); err != nil {
		t.Fatal(err)
	}
	revoked := thumbnailRequest(f, share.Token, etag)
	if revoked.Code != http.StatusNotFound {
		t.Fatalf("revoked video thumbnail = %d, want 404", revoked.Code)
	}
}
