package service

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"image"
	_ "image/gif"
	"image/jpeg"
	_ "image/png"
	"io"

	"golang.org/x/image/draw"
)

var ErrThumbnailNotModified = errors.New("thumbnail not modified")

type Thumbnail struct {
	Content []byte
	ETag    string
}

type ThumbnailService interface {
	Render(context.Context, string, int, int, string) (*Thumbnail, error)
}

type thumbnailService struct {
	files FileStreamer
}

func NewThumbnailService(files FileStreamer) ThumbnailService {
	return &thumbnailService{files: files}
}

func (s *thumbnailService) Render(
	ctx context.Context,
	path string,
	width int,
	height int,
	ifNoneMatch string,
) (*Thumbnail, error) {
	file, info, err := s.files.OpenFile(ctx, path)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	etag := fmt.Sprintf(`W/"%x-%x-%dx%d"`, info.ModTime.UnixNano(), info.Size, width, height)
	if ifNoneMatch == etag {
		return nil, ErrThumbnailNotModified
	}

	configuration, _, err := image.DecodeConfig(file)
	if err != nil {
		return nil, fmt.Errorf("decode image config: %w", err)
	}
	if configuration.Width > 8192 || configuration.Height > 8192 {
		return nil, errors.New("image dimensions exceed thumbnail limit")
	}
	if _, err := file.Seek(0, io.SeekStart); err != nil {
		return nil, err
	}
	source, _, err := image.Decode(file)
	if err != nil {
		return nil, fmt.Errorf("decode image: %w", err)
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	targetWidth, targetHeight := fitDimensions(
		source.Bounds().Dx(),
		source.Bounds().Dy(),
		width,
		height,
	)
	target := image.NewRGBA(image.Rect(0, 0, targetWidth, targetHeight))
	draw.CatmullRom.Scale(target, target.Bounds(), source, source.Bounds(), draw.Over, nil)

	var output bytes.Buffer
	if err := jpeg.Encode(&output, target, &jpeg.Options{Quality: 82}); err != nil {
		return nil, fmt.Errorf("encode thumbnail: %w", err)
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	return &Thumbnail{Content: output.Bytes(), ETag: etag}, nil
}

func fitDimensions(sourceWidth, sourceHeight, maxWidth, maxHeight int) (int, int) {
	if sourceWidth <= 0 || sourceHeight <= 0 {
		return 1, 1
	}
	if sourceWidth <= maxWidth && sourceHeight <= maxHeight {
		return sourceWidth, sourceHeight
	}

	widthRatio := float64(maxWidth) / float64(sourceWidth)
	heightRatio := float64(maxHeight) / float64(sourceHeight)
	ratio := min(widthRatio, heightRatio)
	return max(1, int(float64(sourceWidth)*ratio)), max(1, int(float64(sourceHeight)*ratio))
}
