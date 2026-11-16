package service

import (
	"bytes"
	"compress/gzip"
	"context"
	"encoding/binary"
	"errors"
	"image/jpeg"
	"io"
	"os"
	"runtime"
	"testing"
	"time"
)

func TestWebPHuffmanTablesWaitForMemory(t *testing.T) {
	for _, test := range []struct {
		name  string
		width int
	}{
		{"webp-huffman-groups.webp.gz", 149},
		{"webp-alpha-huffman-groups.webp.gz", 149},
		{"webp-large-huffman-groups.webp.gz", 192},
	} {
		t.Run(test.name, func(t *testing.T) { testWebPHuffmanTablesWaitForMemory(t, test.name, test.width) })
	}
}

func testWebPHuffmanTablesWaitForMemory(t *testing.T, fixture string, wantWidth int) {
	t.Helper()
	content := compressedWebPFixture(t, fixture)
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
	if err != nil || decoded.Width != wantWidth || decoded.Height != 144 {
		t.Fatalf("many-group thumbnail = %+v, %v; want %dx144", decoded, err, wantWidth)
	}
}

func compressedWebPFixture(t *testing.T, fixture string) []byte {
	t.Helper()
	compressed, err := os.ReadFile("testdata/" + fixture)
	if err != nil {
		t.Fatal(err)
	}
	reader, err := gzip.NewReader(bytes.NewReader(compressed))
	if err != nil {
		t.Fatal(err)
	}
	defer reader.Close()
	content, err := io.ReadAll(reader)
	if err != nil {
		t.Fatal(err)
	}
	return content
}

func TestDuplicateWebPAlphaRejectedBeforePartialDecode(t *testing.T) {
	content := compressedWebPFixture(t, "webp-alpha-huffman-groups.webp.gz")
	position := bytes.LastIndex(content, []byte("VP8 "))
	if position < 0 {
		t.Fatal("fixture has no lossy image chunk")
	}
	var malformed bytes.Buffer
	malformed.Write(content[:position])
	malformed.Write([]byte{'A', 'L', 'P', 'H', 1, 0, 0, 0, 0, 0})
	malformed.Write(content[position:])
	data := malformed.Bytes()
	binary.LittleEndian.PutUint32(data[4:8], uint32(len(data)-8))
	fsys, files := newThumbnailTestFiles(t)
	if err := fsys.WriteFile("/data/media/duplicate.webp", data, 0o644); err != nil {
		t.Fatal(err)
	}
	var before, after runtime.MemStats
	runtime.ReadMemStats(&before)
	_, err := NewThumbnailService(files).Render(context.Background(), "media/duplicate.webp", 224, 144, "")
	runtime.ReadMemStats(&after)
	if !errors.Is(err, ErrThumbnailUnsupported) {
		t.Fatalf("duplicate alpha = %v", err)
	}
	if allocated := after.TotalAlloc - before.TotalAlloc; allocated > 8<<20 {
		t.Fatalf("malformed alpha triggered %d bytes of partial decoding", allocated)
	}
}

func TestOrdinaryWebPThumbnails(t *testing.T) {
	for _, test := range []struct {
		name                                 string
		width, height, wantWidth, wantHeight int
	}{
		{"webp-1024x1024.webp", 1200, 1200, 1024, 1024},
		{"webp-2048x1536.webp", 512, 512, 512, 384},
		{"webp-4096x3072.webp", 1200, 1200, 1200, 900},
	} {
		t.Run(test.name, func(t *testing.T) {
			content, err := os.ReadFile("testdata/" + test.name)
			if err != nil {
				t.Fatal(err)
			}
			fsys, files := newThumbnailTestFiles(t)
			if err := fsys.WriteFile("/data/media/ordinary.webp", content, 0o644); err != nil {
				t.Fatal(err)
			}
			file, info, err := files.OpenFile(context.Background(), "media/ordinary.webp")
			if err != nil {
				t.Fatal(err)
			}
			defer file.Close()
			thumbnail, err := renderImageThumbnail(context.Background(), file, info, test.width, test.height)
			if err != nil {
				t.Fatal(err)
			}
			decoded, err := jpeg.DecodeConfig(bytes.NewReader(thumbnail))
			if err != nil || decoded.Width != test.wantWidth || decoded.Height != test.wantHeight {
				t.Fatalf("normal WebP thumbnail = %+v, %v", decoded, err)
			}
		})
	}
}

func TestOpaqueWebPDoesNotWaitForLosslessTables(t *testing.T) {
	content, err := os.ReadFile("testdata/webp-opaque.webp")
	if err != nil {
		t.Fatal(err)
	}
	fsys, files := newThumbnailTestFiles(t)
	if err := fsys.WriteFile("/data/media/ordinary.webp", content, 0o644); err != nil {
		t.Fatal(err)
	}
	occupied := int64(maxThumbnailRenderBytes - (16 << 20))
	if err := thumbnailMemory.Acquire(context.Background(), occupied); err != nil {
		t.Fatal(err)
	}
	defer thumbnailMemory.Release(occupied)
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	if _, err := NewThumbnailService(files).Render(ctx, "media/ordinary.webp", 224, 144, ""); err != nil {
		t.Fatal(err)
	}
}
