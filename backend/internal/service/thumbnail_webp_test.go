package service

import (
	"bytes"
	"compress/gzip"
	"context"
	"errors"
	"image/jpeg"
	"io"
	"os"
	"runtime"
	"testing"
	"time"
)

func TestWebPHuffmanTablesWaitForMemory(t *testing.T) {
	compressed, err := os.Open("testdata/webp-huffman-groups.webp.gz")
	if err != nil {
		t.Fatal(err)
	}
	defer compressed.Close()
	reader, err := gzip.NewReader(compressed)
	if err != nil {
		t.Fatal(err)
	}
	defer reader.Close()
	content, err := io.ReadAll(reader)
	if err != nil {
		t.Fatal(err)
	}
	fsys, files := newThumbnailTestFiles(t)
	// Memory accounting must follow the decoded format even when the extension is misleading.
	if err := fsys.WriteFile("/data/media/tables.png", content, 0o644); err != nil {
		t.Fatal(err)
	}
	renderer := NewThumbnailService(files)
	func() {
		// Leave 192 MiB available: enough for pixels, but not the decoder's construction buffers.
		occupied := int64(maxThumbnailRenderBytes - (192 << 20))
		if err := thumbnailMemory.Acquire(context.Background(), occupied); err != nil {
			t.Fatal(err)
		}
		defer thumbnailMemory.Release(occupied)
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Millisecond)
		defer cancel()
		var before, after runtime.MemStats
		runtime.ReadMemStats(&before)
		_, err := renderer.Render(ctx, "media/tables.png", 224, 144, "")
		runtime.ReadMemStats(&after)
		if !errors.Is(err, context.DeadlineExceeded) {
			t.Fatalf("memory wait = %v, want cancellation", err)
		}
		if allocated := after.TotalAlloc - before.TotalAlloc; allocated > 8<<20 {
			t.Fatalf("decoder allocated %d bytes before memory admission", allocated)
		}
	}()
	thumbnail, err := renderer.Render(context.Background(), "media/tables.png", 224, 144, "")
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := jpeg.DecodeConfig(bytes.NewReader(thumbnail.Content))
	if err != nil || decoded.Width != 149 || decoded.Height != 144 {
		t.Fatalf("many-group thumbnail = %+v, %v; want 149x144", decoded, err)
	}
}
