package service

import (
	"bytes"
	"context"
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
