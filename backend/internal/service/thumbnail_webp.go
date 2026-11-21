package service

import (
	"context"
	"fmt"
	"io"

	"golang.org/x/image/riff"
)

const (
	// x/image v0.47 allows 2600 groups. Retained nodes, allocator padding, and LUTs fit in 160 MiB;
	// constructing those trees also allocates temporary code-length and canonical-code buffers.
	webpRetainedTableBytes = 160 << 20
	webpTableBuildBytes    = 224 << 20
)

type webpAlphaMode uint8

const (
	webpAlphaAbsent webpAlphaMode = iota
	webpAlphaRaw
	webpAlphaLossless
)

// WebP's chunk types select the decoder; VP8L also handles compressed ALPH data.
func webPDecoderMemory(ctx context.Context, file File, width, height int) (int64, error) {
	if _, err := file.Seek(0, io.SeekStart); err != nil {
		return 0, err
	}
	form, chunks, err := riff.NewReader(io.LimitReader(file, maxThumbnailSourceBytes))
	if err != nil {
		return 0, fmt.Errorf("%w: %v", ErrThumbnailUnsupported, err)
	}
	if string(form[:]) != "WEBP" {
		return 0, ErrThumbnailUnsupported
	}
	alpha := webpAlphaAbsent
	for {
		if err := ctx.Err(); err != nil {
			return 0, err
		}
		kind, size, chunk, err := chunks.Next()
		if err != nil {
			return 0, fmt.Errorf("%w: %v", ErrThumbnailUnsupported, err)
		}
		if size > maxThumbnailSourceBytes {
			return 0, ErrThumbnailTooLarge
		}
		switch string(kind[:]) {
		case "VP8L":
			return webPLosslessMemory(width, height), nil
		case "ALPH":
			if alpha != webpAlphaAbsent {
				return 0, ErrThumbnailUnsupported
			}
			var header [1]byte
			if _, err := io.ReadFull(chunk, header[:]); err != nil {
				return 0, fmt.Errorf("%w: %v", ErrThumbnailUnsupported, err)
			}
			switch header[0] & 3 {
			case 0:
				alpha = webpAlphaRaw
			case 1:
				alpha = webpAlphaLossless
			default:
				return 0, ErrThumbnailUnsupported
			}
		case "VP8 ":
			// The lossy decoder retains encoded partition data as well as pixel buffers.
			memory := int64(size) + int64(width)*int64(height)*8 + (2 << 20)
			if alpha == webpAlphaLossless {
				memory = max(memory, webPLosslessMemory(width, height))
			}
			return memory, nil
		}
	}
}

func webPLosslessMemory(width, height int) int64 {
	pixels := int64(width) * int64(height)
	// Predictor, cross-color, and Huffman metadata images use tiles of at least 4x4.
	metadata := ((int64(width) + 3) / 4) * ((int64(height) + 3) / 4) * 4 * 3
	// Tree construction precedes pixel allocation. Trees die before inverse transforms;
	// the renderer separately reserves scaling workspace after Decode returns.
	return max(webpTableBuildBytes+metadata,
		webpRetainedTableBytes+pixels*4+metadata,
		pixels*8+metadata) + (2 << 20)
}
