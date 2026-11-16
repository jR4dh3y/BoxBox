package service

import (
	"bufio"
	"bytes"
	"context"
	"encoding/binary"
	"fmt"
	"image"
	_ "image/gif"
	"image/jpeg"
	_ "image/png"
	"io"
	"path/filepath"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"golang.org/x/image/draw"
	_ "golang.org/x/image/webp"
	"golang.org/x/sync/semaphore"
)

// Leave room in the shipped 512 MiB container for caches, other requests, and GC.
const maxThumbnailRenderBytes = 256 << 20

var thumbnailMemory = semaphore.NewWeighted(maxThumbnailRenderBytes)

// RendersImageThumbnail reports whether a file name has an extension the pure-Go renderer decodes.
func RendersImageThumbnail(name string) bool {
	switch strings.ToLower(filepath.Ext(name)) {
	case ".jpg", ".jpeg", ".png", ".gif", ".webp":
		return true
	}
	return false
}

func renderImageThumbnail(ctx context.Context, file File, info *model.FileInfo, width, height int) ([]byte, error) {
	if info.Size < 0 || info.Size > maxThumbnailSourceBytes {
		return nil, ErrThumbnailTooLarge
	}
	configuration, format, err := image.DecodeConfig(file)
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrThumbnailUnsupported, err)
	}
	if !thumbnailSourceWithinLimits(configuration.Width, configuration.Height) {
		return nil, ErrThumbnailTooLarge
	}
	// Eight bytes per source pixel covers 16-bit PNGs and JPEG color conversion.
	// Bound workspace for any frame inside the canvas; GIF frames can have a different aspect ratio.
	outputWidth, outputHeight := int64(min(width, configuration.Width)), int64(min(height, configuration.Height))
	scratchPixels := min(outputWidth*int64(configuration.Height), outputHeight*int64(configuration.Width))
	sourceWidth, sourceHeight, bytesPerPixel := int64(configuration.Width), int64(configuration.Height), int64(8)
	if format == "jpeg" {
		progressive, err := progressiveJPEG(file)
		if err != nil {
			return nil, fmt.Errorf("%w: %v", ErrThumbnailUnsupported, err)
		}
		// JPEG pads MCU blocks and retains up to four int32 coefficient planes during progressive decoding.
		sourceWidth, sourceHeight = (sourceWidth+31)/32*32, (sourceHeight+31)/32*32
		if progressive {
			bytesPerPixel += 16
		}
	}
	// Each CatmullRom scratch pixel holds four float64 channels.
	memory := sourceWidth*sourceHeight*bytesPerPixel +
		scratchPixels*32 + outputWidth*outputHeight*4 + (2 << 20)
	if memory > maxThumbnailRenderBytes {
		return nil, ErrThumbnailTooLarge
	}
	if err := thumbnailMemory.Acquire(ctx, memory); err != nil {
		return nil, err
	}
	defer thumbnailMemory.Release(memory)
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

// DecodeConfig omits JPEG scan mode, which determines whether the decoder retains coefficient planes.
func progressiveJPEG(file File) (bool, error) {
	if _, err := file.Seek(2, io.SeekStart); err != nil {
		return false, err
	}
	r := bufio.NewReader(file)
	for {
		b, err := r.ReadByte()
		if err != nil {
			return false, err
		}
		if b != 0xff {
			continue
		}
		for b == 0xff {
			b, err = r.ReadByte()
			if err != nil {
				return false, err
			}
		}
		switch b {
		case 0xc0, 0xc1:
			return false, nil
		case 0xc2:
			return true, nil
		case 0x00, 0x01, 0xd0, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7:
			continue
		}
		var length [2]byte
		if _, err := io.ReadFull(r, length[:]); err != nil {
			return false, err
		}
		n := int64(binary.BigEndian.Uint16(length[:])) - 2
		if n < 0 {
			return false, fmt.Errorf("invalid JPEG segment length")
		}
		if _, err := io.CopyN(io.Discard, r, n); err != nil {
			return false, err
		}
	}
}
