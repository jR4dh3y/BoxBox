package service

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/binary"
	"errors"
	"fmt"
	"hash/crc32"
	"image"
	"image/color"
	"image/gif"
	"image/jpeg"
	"image/png"
	"sync"
	"testing"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

const generatedWebPFixture = "UklGRpIAAABXRUJQVlA4IIYAAACQBQCdASpAACgAPpFAnUolo6KhpXgLMLASCWYAxfgzeV7ygZB//94gAWCy5uol7qIfK+gMgAD+8ALNp9XZ9yvD3ZwX/+0iD+pEH8DMYIhNBJLFKGgeFiWF8ZFub0j+WZit7Ipqm937TRou6ctPcUqpzqN2KQJ8yRowlKbBIsbnodv/yR4AAA=="

func TestThumbnailServiceDecodesSupportedFormats(t *testing.T) {
	webp, err := base64.StdEncoding.DecodeString(generatedWebPFixture)
	if err != nil {
		t.Fatal(err)
	}
	fixtures := []struct {
		name    string
		content []byte
	}{
		{name: "photo.jpg", content: generatedImage(t, "jpeg", 640, 400, color.NRGBA{R: 180, G: 50, B: 30, A: 255})},
		{name: "photo.png", content: generatedImage(t, "png", 640, 400, color.NRGBA{R: 30, G: 180, B: 50, A: 255})},
		{name: "photo.gif", content: generatedImage(t, "gif", 640, 400, color.NRGBA{R: 30, G: 50, B: 180, A: 255})},
		{name: "photo.webp", content: webp},
	}

	fsys, files := newThumbnailTestFiles(t)
	for _, fixture := range fixtures {
		if err := fsys.WriteFile("/data/media/"+fixture.name, fixture.content, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	renderer := NewThumbnailService(files)
	for _, fixture := range fixtures {
		t.Run(fixture.name, func(t *testing.T) {
			thumbnail, err := renderer.Render(context.Background(), "media/"+fixture.name, 224, 144, "")
			if err != nil {
				t.Fatal(err)
			}
			decoded, err := jpeg.Decode(bytes.NewReader(thumbnail.Content))
			if err != nil {
				t.Fatalf("thumbnail is not a JPEG: %v", err)
			}
			if decoded.Bounds().Dx() > 224 || decoded.Bounds().Dy() > 144 {
				t.Fatalf("thumbnail bounds = %v, exceed 224x144", decoded.Bounds())
			}
			if thumbnail.ETag == "" {
				t.Fatal("expected an etag")
			}
		})
	}
}

func TestThumbnailServiceRendersCameraPhotoAboveLegacyPixelLimit(t *testing.T) {
	fsys, files := newThumbnailTestFiles(t)
	photo := generatedImage(t, "jpeg", 6000, 4000, color.NRGBA{R: 92, G: 124, B: 176, A: 255})
	if err := fsys.WriteFile("/data/media/camera.jpg", photo, 0o644); err != nil {
		t.Fatal(err)
	}

	thumbnail, err := NewThumbnailService(files).Render(context.Background(), "media/camera.jpg", 224, 144, "")
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := jpeg.Decode(bytes.NewReader(thumbnail.Content))
	if err != nil {
		t.Fatalf("thumbnail is not a JPEG: %v", err)
	}
	if decoded.Bounds().Dx() != 216 || decoded.Bounds().Dy() != 144 {
		t.Fatalf("thumbnail bounds = %v, want 216x144 aspect-fit", decoded.Bounds())
	}
}

func TestThumbnailServiceRejectsSourcesOutsideResourceLimits(t *testing.T) {
	t.Run("pixel count", func(t *testing.T) {
		fsys, files := newThumbnailTestFiles(t)
		if err := fsys.WriteFile("/data/media/oversized.png", pngHeader(6000, 6000), 0o644); err != nil {
			t.Fatal(err)
		}
		_, err := NewThumbnailService(files).Render(context.Background(), "media/oversized.png", 224, 144, "")
		if !errors.Is(err, ErrThumbnailTooLarge) {
			t.Fatalf("expected thumbnail limit error, got %v", err)
		}
	})

	t.Run("compressed bytes", func(t *testing.T) {
		fsys, files := newThumbnailTestFiles(t)
		content := generatedImage(t, "png", 8, 8, color.NRGBA{R: 90, G: 110, B: 130, A: 255})
		if err := fsys.WriteFile("/data/media/large-file.png", content, 0o644); err != nil {
			t.Fatal(err)
		}
		oversized := int64(maxThumbnailSourceBytes + 1)
		fileInfo := &thumbnailMetadataOverride{
			FileStreamer: files,
			size:         &oversized,
		}
		_, err := NewThumbnailService(fileInfo).Render(context.Background(), "media/large-file.png", 224, 144, "")
		if !errors.Is(err, ErrThumbnailTooLarge) {
			t.Fatalf("expected source byte limit error, got %v", err)
		}
	})
}

func TestThumbnailServiceCachesBySourceMetadataAndDimensions(t *testing.T) {
	fsys, files := newThumbnailTestFiles(t)
	firstSource := generatedImage(t, "png", 80, 40, color.NRGBA{R: 210, G: 30, B: 20, A: 255})
	if err := fsys.WriteFile("/data/media/photo.png", firstSource, 0o644); err != nil {
		t.Fatal(err)
	}
	stableSize := int64(80)
	metadata := &thumbnailMetadataOverride{
		FileStreamer: files,
		modTime:      time.Unix(100, 0),
		size:         &stableSize,
	}
	renderer := NewThumbnailService(metadata).(*thumbnailService)

	first, err := renderer.Render(context.Background(), "media/photo.png", 64, 48, "")
	if err != nil {
		t.Fatal(err)
	}
	repeated, err := renderer.Render(context.Background(), "media/photo.png", 64, 48, "")
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(repeated.Content, first.Content) || repeated.ETag != first.ETag {
		t.Fatal("repeat request did not return the cached thumbnail")
	}
	if _, err := renderer.Render(context.Background(), "media/photo.png", 64, 48, first.ETag); !errors.Is(err, ErrThumbnailNotModified) {
		t.Fatalf("conditional repeat error = %v, want not modified", err)
	}
	if count, _ := renderer.cache.stats(); count != 1 {
		t.Fatalf("cache entry count = %d, want 1", count)
	}

	secondSource := generatedImage(t, "png", 80, 40, color.NRGBA{R: 20, G: 40, B: 220, A: 255})
	if err := fsys.WriteFile("/data/media/photo.png", secondSource, 0o644); err != nil {
		t.Fatal(err)
	}
	metadata.modTime = time.Unix(101, 0)
	updated, err := renderer.Render(context.Background(), "media/photo.png", 64, 48, "")
	if err != nil {
		t.Fatal(err)
	}
	if updated.ETag == first.ETag || bytes.Equal(updated.Content, first.Content) {
		t.Fatal("source modification reused the previous thumbnail")
	}
	if count, _ := renderer.cache.stats(); count != 1 {
		t.Fatalf("stale source cache entry was not invalidated, count = %d", count)
	}

	otherSize, err := renderer.Render(context.Background(), "media/photo.png", 32, 24, "")
	if err != nil {
		t.Fatal(err)
	}
	if otherSize.ETag == updated.ETag {
		t.Fatal("different thumbnail dimensions shared an etag")
	}
	if count, _ := renderer.cache.stats(); count != 2 {
		t.Fatalf("dimension-specific cache entry count = %d, want 2", count)
	}
}

func TestThumbnailCacheRespectsEntryAndByteBounds(t *testing.T) {
	byCount := newThumbnailCache()
	for index := 0; index < maxThumbnailCacheEntries+8; index++ {
		byCount.add(thumbnailCacheKey{path: fmt.Sprintf("image-%d", index)}, &Thumbnail{Content: []byte{1}})
	}
	if count, _ := byCount.stats(); count != maxThumbnailCacheEntries {
		t.Fatalf("entry bound = %d, want %d", count, maxThumbnailCacheEntries)
	}

	byBytes := newThumbnailCache()
	content := bytes.Repeat([]byte{1}, 1<<20)
	for index := 0; index < maxThumbnailCacheBytes/(1<<20)+1; index++ {
		byBytes.add(thumbnailCacheKey{path: fmt.Sprintf("large-%d", index)}, &Thumbnail{Content: content})
	}
	count, size := byBytes.stats()
	if count != maxThumbnailCacheBytes/(1<<20) || size != maxThumbnailCacheBytes {
		t.Fatalf("byte bounded cache = (%d entries, %d bytes), want (%d, %d)", count, size, maxThumbnailCacheBytes/(1<<20), maxThumbnailCacheBytes)
	}
}

func TestThumbnailServiceHandlesConcurrentCacheAccess(t *testing.T) {
	fsys, files := newThumbnailTestFiles(t)
	if err := fsys.WriteFile(
		"/data/media/photo.png",
		generatedImage(t, "png", 640, 400, color.NRGBA{R: 90, G: 120, B: 180, A: 255}),
		0o644,
	); err != nil {
		t.Fatal(err)
	}
	renderer := NewThumbnailService(files).(*thumbnailService)
	const requests = 8
	results := make(chan error, requests)
	var workers sync.WaitGroup
	workers.Add(requests)
	for index := 0; index < requests; index++ {
		go func() {
			defer workers.Done()
			thumbnail, err := renderer.Render(context.Background(), "media/photo.png", 224, 144, "")
			if err == nil && len(thumbnail.Content) == 0 {
				err = errors.New("render returned empty content")
			}
			results <- err
		}()
	}
	workers.Wait()
	close(results)
	for err := range results {
		if err != nil {
			t.Error(err)
		}
	}
	if count, _ := renderer.cache.stats(); count != 1 {
		t.Fatalf("concurrent cache entry count = %d, want 1", count)
	}
}

func newThumbnailTestFiles(t *testing.T) (*filesystem.AferoFS, FileService) {
	t.Helper()
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/data/media", 0o755); err != nil {
		t.Fatal(err)
	}
	files := NewFileService(fsys, FileServiceConfig{MountPoints: []model.MountPoint{{
		Name: "media",
		Path: "/data/media",
	}}})
	return fsys, files
}

func generatedImage(t *testing.T, format string, width, height int, fill color.NRGBA) []byte {
	t.Helper()
	img := image.NewNRGBA(image.Rect(0, 0, width, height))
	for index := 0; index < len(img.Pix); index += 4 {
		img.Pix[index] = fill.R
		img.Pix[index+1] = fill.G
		img.Pix[index+2] = fill.B
		img.Pix[index+3] = fill.A
	}
	var output bytes.Buffer
	var err error
	switch format {
	case "jpeg":
		err = jpeg.Encode(&output, img, &jpeg.Options{Quality: 88})
	case "png":
		err = png.Encode(&output, img)
	case "gif":
		err = gif.Encode(&output, img, nil)
	default:
		t.Fatalf("unsupported generated test image format %q", format)
	}
	if err != nil {
		t.Fatal(err)
	}
	return output.Bytes()
}

type thumbnailMetadataOverride struct {
	FileStreamer
	modTime time.Time
	size    *int64
}

func (f *thumbnailMetadataOverride) OpenFile(ctx context.Context, path string) (File, *model.FileInfo, error) {
	file, info, err := f.FileStreamer.OpenFile(ctx, path)
	if err != nil {
		return nil, nil, err
	}
	if !f.modTime.IsZero() {
		info.ModTime = f.modTime
	}
	if f.size != nil {
		info.Size = *f.size
	}
	return file, info, nil
}

func pngHeader(width, height uint32) []byte {
	var output bytes.Buffer
	output.Write([]byte("\x89PNG\r\n\x1a\n"))
	data := make([]byte, 13)
	binary.BigEndian.PutUint32(data[0:4], width)
	binary.BigEndian.PutUint32(data[4:8], height)
	data[8] = 8
	data[9] = 2
	writePNGChunk(&output, "IHDR", data)
	return output.Bytes()
}

func writePNGChunk(output *bytes.Buffer, chunkType string, data []byte) {
	_ = binary.Write(output, binary.BigEndian, uint32(len(data)))
	payload := append([]byte(chunkType), data...)
	output.Write(payload)
	_ = binary.Write(output, binary.BigEndian, crc32.ChecksumIEEE(payload))
}
