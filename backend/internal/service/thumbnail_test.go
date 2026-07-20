package service

import (
	"bytes"
	"context"
	"encoding/binary"
	"errors"
	"hash/crc32"
	"image"
	"image/color"
	"image/png"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

func TestThumbnailServiceRendersAndValidatesETag(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/data/media", 0o755); err != nil {
		t.Fatal(err)
	}
	var source bytes.Buffer
	img := image.NewRGBA(image.Rect(0, 0, 800, 400))
	for x := 0; x < 800; x++ {
		for y := 0; y < 400; y++ {
			img.Set(x, y, color.RGBA{R: 220, A: 255})
		}
	}
	if err := png.Encode(&source, img); err != nil {
		t.Fatal(err)
	}
	if err := fsys.WriteFile("/data/media/photo.png", source.Bytes(), 0o644); err != nil {
		t.Fatal(err)
	}

	files := NewFileService(fsys, FileServiceConfig{MountPoints: []model.MountPoint{{
		Name: "media", Path: "/data/media",
	}}})
	service := NewThumbnailService(files)
	thumbnail, err := service.Render(context.Background(), "media/photo.png", 224, 144, "")
	if err != nil {
		t.Fatal(err)
	}
	if len(thumbnail.Content) == 0 || thumbnail.ETag == "" {
		t.Fatal("expected encoded thumbnail and etag")
	}
	if _, err := service.Render(context.Background(), "media/photo.png", 224, 144, thumbnail.ETag); err != ErrThumbnailNotModified {
		t.Fatalf("expected not modified, got %v", err)
	}
}

func TestThumbnailServiceRejectsExcessivePixelCountBeforeDecode(t *testing.T) {
	fsys := filesystem.NewMemMapFS()
	if err := fsys.MkdirAll("/data/media", 0o755); err != nil {
		t.Fatal(err)
	}
	if err := fsys.WriteFile(
		"/data/media/oversized.png",
		pngHeader(4096, 4096),
		0o644,
	); err != nil {
		t.Fatal(err)
	}

	files := NewFileService(fsys, FileServiceConfig{MountPoints: []model.MountPoint{{
		Name: "media", Path: "/data/media",
	}}})
	_, err := NewThumbnailService(files).Render(
		context.Background(),
		"media/oversized.png",
		224,
		144,
		"",
	)
	if !errors.Is(err, ErrThumbnailTooLarge) {
		t.Fatalf("expected thumbnail limit error, got %v", err)
	}
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
