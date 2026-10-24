package service

import (
	"bytes"
	"container/list"
	"context"
	"crypto/sha256"
	"errors"
	"fmt"
	"image"
	_ "image/gif"
	"image/jpeg"
	_ "image/png"
	"io"
	"sync"

	"golang.org/x/image/draw"
	_ "golang.org/x/image/webp"
)

var (
	ErrThumbnailNotModified = errors.New("thumbnail not modified")
	ErrThumbnailTooLarge    = errors.New("thumbnail source exceeds resource limits")
)

const (
	maxThumbnailSourceBytes = 64 << 20
	// At most 128 MiB per 32 MP RGBA decode; two render slots bound peak source pixels.
	maxThumbnailSourcePixels = 32_000_000
	maxThumbnailDimension    = 8192
	maxThumbnailOutput       = 512
	maxConcurrentThumbnails  = 2
	maxThumbnailCacheEntries = 128
	maxThumbnailCacheBytes   = 16 << 20
)

type Thumbnail struct {
	Content []byte
	ETag    string
}

type ThumbnailService interface {
	Render(context.Context, string, int, int, string) (*Thumbnail, error)
}

type thumbnailService struct {
	files FileStreamer
	slots chan struct{}
	cache *thumbnailCache
}

func NewThumbnailService(files FileStreamer) ThumbnailService {
	return &thumbnailService{
		files: files,
		slots: make(chan struct{}, maxConcurrentThumbnails),
		cache: newThumbnailCache(),
	}
}

func (s *thumbnailService) Render(
	ctx context.Context,
	path string,
	width int,
	height int,
	ifNoneMatch string,
) (*Thumbnail, error) {
	select {
	case s.slots <- struct{}{}:
		defer func() { <-s.slots }()
	case <-ctx.Done():
		return nil, ctx.Err()
	}

	file, info, err := s.files.OpenFile(ctx, path)
	if err != nil {
		return nil, err
	}
	defer file.Close()

	width = min(maxThumbnailOutput, max(16, width))
	height = min(maxThumbnailOutput, max(16, height))
	key := thumbnailCacheKey{
		path:    path,
		modTime: info.ModTime.UnixNano(),
		size:    info.Size,
		width:   width,
		height:  height,
	}
	etag := thumbnailETag(key)
	if cached, ok := s.cache.get(key); ok {
		if ifNoneMatch == etag {
			return nil, ErrThumbnailNotModified
		}
		return cached, nil
	}
	if ifNoneMatch == etag {
		return nil, ErrThumbnailNotModified
	}
	if info.Size < 0 || info.Size > maxThumbnailSourceBytes {
		return nil, ErrThumbnailTooLarge
	}

	configuration, _, err := image.DecodeConfig(file)
	if err != nil {
		return nil, fmt.Errorf("decode image config: %w", err)
	}
	if !thumbnailSourceWithinLimits(configuration.Width, configuration.Height) {
		return nil, ErrThumbnailTooLarge
	}

	if _, err := file.Seek(0, io.SeekStart); err != nil {
		return nil, err
	}
	source, _, err := image.Decode(file)
	if err != nil {
		return nil, fmt.Errorf("decode image: %w", err)
	}
	if !thumbnailSourceWithinLimits(source.Bounds().Dx(), source.Bounds().Dy()) {
		return nil, ErrThumbnailTooLarge
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
	thumbnail := &Thumbnail{Content: output.Bytes(), ETag: etag}
	s.cache.add(key, thumbnail)
	return thumbnail, nil
}

func thumbnailSourceWithinLimits(width, height int) bool {
	if width <= 0 || height <= 0 || width > maxThumbnailDimension || height > maxThumbnailDimension {
		return false
	}
	return int64(width)*int64(height) <= maxThumbnailSourcePixels
}

func fitDimensions(sourceWidth, sourceHeight, maxWidth, maxHeight int) (int, int) {
	if sourceWidth <= 0 || sourceHeight <= 0 {
		return 1, 1
	}
	if sourceWidth <= maxWidth && sourceHeight <= maxHeight {
		return sourceWidth, sourceHeight
	}

	if sourceWidth*maxHeight > sourceHeight*maxWidth {
		return maxWidth, max(1, sourceHeight*maxWidth/sourceWidth)
	}
	return max(1, sourceWidth*maxHeight/sourceHeight), maxHeight
}

type thumbnailCacheKey struct {
	path    string
	modTime int64
	size    int64
	width   int
	height  int
}

type thumbnailCacheEntry struct {
	key       thumbnailCacheKey
	thumbnail Thumbnail
}

type thumbnailCache struct {
	mu      sync.Mutex
	entries map[thumbnailCacheKey]*list.Element
	lru     *list.List
	bytes   int64
}

func newThumbnailCache() *thumbnailCache {
	return &thumbnailCache{
		entries: make(map[thumbnailCacheKey]*list.Element),
		lru:     list.New(),
	}
}

func thumbnailETag(key thumbnailCacheKey) string {
	identity := fmt.Sprintf("%s\x00%d\x00%d\x00%d\x00%d", key.path, key.modTime, key.size, key.width, key.height)
	sum := sha256.Sum256([]byte(identity))
	return fmt.Sprintf(`W/"%x"`, sum[:16])
}

func (c *thumbnailCache) get(key thumbnailCacheKey) (*Thumbnail, bool) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.removeStalePathEntries(key)

	element, ok := c.entries[key]
	if !ok {
		return nil, false
	}
	c.lru.MoveToFront(element)
	entry := element.Value.(thumbnailCacheEntry)
	thumbnail := entry.thumbnail
	thumbnail.Content = bytes.Clone(thumbnail.Content)
	return &thumbnail, true
}

func (c *thumbnailCache) add(key thumbnailCacheKey, thumbnail *Thumbnail) {
	size := int64(len(thumbnail.Content))
	if size == 0 || size > maxThumbnailCacheBytes {
		return
	}

	c.mu.Lock()
	defer c.mu.Unlock()
	c.removeStalePathEntries(key)
	if existing, ok := c.entries[key]; ok {
		c.remove(existing)
	}

	entry := thumbnailCacheEntry{key: key, thumbnail: Thumbnail{
		Content: bytes.Clone(thumbnail.Content),
		ETag:    thumbnail.ETag,
	}}
	c.entries[key] = c.lru.PushFront(entry)
	c.bytes += size
	for len(c.entries) > maxThumbnailCacheEntries || c.bytes > maxThumbnailCacheBytes {
		c.remove(c.lru.Back())
	}
}

func (c *thumbnailCache) removeStalePathEntries(key thumbnailCacheKey) {
	for element := c.lru.Back(); element != nil; {
		previous := element.Prev()
		entry := element.Value.(thumbnailCacheEntry)
		if entry.key.path == key.path && (entry.key.modTime != key.modTime || entry.key.size != key.size) {
			c.remove(element)
		}
		element = previous
	}
}

func (c *thumbnailCache) remove(element *list.Element) {
	if element == nil {
		return
	}
	entry := element.Value.(thumbnailCacheEntry)
	delete(c.entries, entry.key)
	c.bytes -= int64(len(entry.thumbnail.Content))
	c.lru.Remove(element)
}

func (c *thumbnailCache) stats() (int, int64) {
	c.mu.Lock()
	defer c.mu.Unlock()
	return len(c.entries), c.bytes
}
