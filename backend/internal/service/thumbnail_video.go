package service

import (
	"context"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"time"
)

const (
	maxVideoThumbnailBytes = 2 << 20
	videoThumbnailTimeout  = 5 * time.Second
	// Include virtual space for the dynamic linker and thread stacks.
	videoAddressSpaceBytes = 320 << 20
)

type videoThumbnailer struct {
	ffmpeg, prlimit string
}

func newVideoThumbnailer() videoThumbnailer {
	if runtime.GOOS != "linux" {
		return videoThumbnailer{}
	}
	ffmpeg, _ := exec.LookPath("ffmpeg")
	prlimit, _ := exec.LookPath("prlimit")
	return videoThumbnailer{ffmpeg: ffmpeg, prlimit: prlimit}
}

func (v videoThumbnailer) available() bool {
	return v.ffmpeg != "" && v.prlimit != ""
}

func videoDemuxer(name string) string {
	switch strings.ToLower(filepath.Ext(name)) {
	case ".mp4", ".m4v", ".mov":
		return "mov"
	case ".mkv", ".webm":
		return "matroska"
	}
	return ""
}

func (v videoThumbnailer) render(ctx context.Context, file File, demuxer string, width, height int) ([]byte, error) {
	if !v.available() {
		return nil, ErrThumbnailUnavailable
	}
	// Hand FFmpeg the confined descriptor. Opening a name again would repeat path resolution.
	source, ok := file.(*os.File)
	if !ok {
		return nil, ErrThumbnailUnsupported
	}
	stat, err := source.Stat()
	if err != nil || !stat.Mode().IsRegular() {
		return nil, ErrThumbnailUnsupported
	}

	ctx, cancel := context.WithTimeout(ctx, videoThumbnailTimeout)
	defer cancel()
	// The FFmpeg child and the Go image decoders use the same rendering budget.
	if err := thumbnailMemory.Acquire(ctx, maxThumbnailRenderBytes); err != nil {
		return nil, ErrThumbnailUnavailable
	}
	defer thumbnailMemory.Release(maxThumbnailRenderBytes)

	// Skip the opening frame when possible. Short clips can end before the seek point.
	for _, timestamp := range []string{"1", "0"} {
		if ctx.Err() != nil {
			break
		}
		if _, err := source.Seek(0, io.SeekStart); err != nil {
			return nil, ErrThumbnailUnsupported
		}
		if content := v.frame(ctx, source, demuxer, width, height, timestamp); len(content) > 0 {
			return content, nil
		}
	}
	return nil, ErrThumbnailUnsupported
}

func (v videoThumbnailer) frame(ctx context.Context, source *os.File, demuxer string, width, height int, timestamp string) []byte {
	// Fit the displayed aspect ratio in the output box and write square pixels.
	scale := fmt.Sprintf("scale=w='max(1,min(iw*sar,min(%d,%d*dar)))':h='max(1,min(ih,min(%d,%d/dar)))',setsar=1",
		width, height, height, width)
	args := []string{
		fmt.Sprintf("--as=%d", videoAddressSpaceBytes), "--cpu=3", "--stack=2097152", "--core=0", "--",
		v.ffmpeg, "-nostdin", "-loglevel", "error", "-max_alloc", "67108864",
		"-filter_threads", "1", "-protocol_whitelist", "fd", "-f", demuxer,
		"-probesize", "8388608", "-analyzeduration", "3000000", "-max_streams", "32",
		"-max_pixels", "32000000", "-threads", "1", "-ss", timestamp,
	}
	if demuxer == "mov" {
		args = append(args, "-enable_drefs", "0")
	}
	args = append(args,
		"-fd", "3", "-i", "fd:", "-map", "0:V:0", "-frames:v", "1",
		"-vf", scale, "-c:v", "mjpeg", "-threads", "1", "-q:v", "4", "-f", "image2pipe", "pipe:1",
	)
	cmd := exec.CommandContext(ctx, v.prlimit, args...)
	// Keep glibc allocator arenas within the child's address-space ceiling.
	cmd.Env = []string{"LANG=C", "MALLOC_ARENA_MAX=1"}
	cmd.ExtraFiles = []*os.File{source}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil
	}
	if err := cmd.Start(); err != nil {
		return nil
	}
	content, err := io.ReadAll(io.LimitReader(stdout, maxVideoThumbnailBytes+1))
	if err != nil || len(content) > maxVideoThumbnailBytes {
		_ = cmd.Process.Kill()
		_ = cmd.Wait()
		return nil
	}
	if err := cmd.Wait(); err != nil {
		return nil
	}
	return content
}
