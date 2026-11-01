package service

import (
	"bytes"
	"context"
	"fmt"
	"image"
	_ "image/gif"
	"image/jpeg"
	_ "image/png"
	"io"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"golang.org/x/image/draw"
	_ "golang.org/x/image/webp"
)

func renderImageThumbnail(ctx context.Context, file File, info *model.FileInfo, width, height int) ([]byte, error) {
	if info.Size < 0 || info.Size > maxThumbnailSourceBytes {
		return nil, ErrThumbnailTooLarge
	}
	configuration, _, err := image.DecodeConfig(file)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrThumbnailUnsupported, err)
	}
	if !thumbnailSourceWithinLimits(configuration.Width, configuration.Height) {
		return nil, ErrThumbnailTooLarge
	}
	if _, err := file.Seek(0, io.SeekStart); err != nil {
		return nil, err
	}
	source, _, err := image.Decode(file)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrThumbnailUnsupported, err)
	}
	if !thumbnailSourceWithinLimits(source.Bounds().Dx(), source.Bounds().Dy()) {
		return nil, ErrThumbnailTooLarge
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	targetWidth, targetHeight := fitDimensions(source.Bounds().Dx(), source.Bounds().Dy(), width, height)
	target := image.NewRGBA(image.Rect(0, 0, targetWidth, targetHeight))
	draw.CatmullRom.Scale(target, target.Bounds(), source, source.Bounds(), draw.Over, nil)
	var output bytes.Buffer
	if err := jpeg.Encode(&output, target, &jpeg.Options{Quality: 82}); err != nil {
		return nil, fmt.Errorf("encode thumbnail: %w", err)
	}
	return output.Bytes(), nil
}
