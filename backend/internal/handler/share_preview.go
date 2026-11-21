package handler

import (
	"context"
	"crypto/sha256"
	"errors"
	"fmt"
	"net/http"
	"net/netip"
	"net/url"
	"strconv"
	"strings"
	"unicode"
	"unicode/utf8"

	"github.com/go-chi/chi/v5"
	"github.com/jR4dh3y/BoxBox/backend/internal/service"
	"github.com/jR4dh3y/BoxBox/backend/internal/static"
	"github.com/rs/zerolog/log"
)

const (
	defaultPreviewImage       = "og-default.png"
	defaultPreviewImageWidth  = 1200
	defaultPreviewImageHeight = 630
	sharePreviewImageSize     = 1200
	maxPreviewNameRunes       = 120
	maxShareTokenBytes        = 128
)

// PreviewDocument is the SPA document that link-preview tags are inserted into.
type PreviewDocument interface {
	ServeIndexWithMeta(w http.ResponseWriter, r *http.Request, meta static.PageMeta)
	Asset(name string) ([]byte, bool)
}

// SharePreviewHandler serves share pages and images for link-preview crawlers.
type SharePreviewHandler struct {
	shares       service.ShareService
	thumbnails   *service.ShareThumbnailService
	document     PreviewDocument
	publicURL    string
	defaultImage []byte
	defaultETag  string
}

// NewSharePreviewHandler builds the handler. publicURL is an origin without a trailing slash, or "" to derive it per request.
func NewSharePreviewHandler(shares service.ShareService, document PreviewDocument, publicURL string) *SharePreviewHandler {
	defaultImage, _ := document.Asset(defaultPreviewImage)
	return &SharePreviewHandler{
		shares:       shares,
		thumbnails:   service.NewShareThumbnailService(shares),
		document:     document,
		publicURL:    publicURL,
		defaultImage: defaultImage,
		defaultETag:  fmt.Sprintf(`"%x"`, sha256.Sum256(defaultImage)),
	}
}

type sharePreviewKind int

const (
	sharePreviewGeneric sharePreviewKind = iota
	sharePreviewFile
	sharePreviewThumbnail
	sharePreviewFolder
)

type sharePreview struct {
	kind    sharePreviewKind
	name    string
	modTime int64
}

// Page serves the share page with tags describing the share.
// GET /s/{token}
func (h *SharePreviewHandler) Page(w http.ResponseWriter, r *http.Request) {
	token := chi.URLParam(r, "token")
	h.document.ServeIndexWithMeta(w, r, sharePageMeta(h.lookup(r.Context(), token), h.origin(r), token))
}

// GenericPage serves the share page with generic tags and no share lookup.
func (h *SharePreviewHandler) GenericPage(w http.ResponseWriter, r *http.Request) {
	h.document.ServeIndexWithMeta(w, r, sharePageMeta(sharePreview{}, h.origin(r), ""))
}

// lookup never fails: any problem yields the generic preview, which cannot tell expired from unknown.
func (h *SharePreviewHandler) lookup(ctx context.Context, token string) (preview sharePreview) {
	if token == "" || len(token) > maxShareTokenBytes {
		return sharePreview{}
	}
	defer func() {
		if recovered := recover(); recovered != nil {
			log.Error().Interface("panic", recovered).Msg("Share preview lookup panicked")
			preview = sharePreview{}
		}
	}()

	info, _, err := h.shares.InfoForRecipient(ctx, token)
	if err != nil {
		if !errors.Is(err, service.ErrShareNotFound) {
			log.Warn().Err(err).Msg("Share preview lookup failed")
		}
		return sharePreview{}
	}
	preview = sharePreview{kind: sharePreviewFile, name: info.Name, modTime: info.ModTime.UnixNano()}
	switch {
	case info.IsDir:
		preview.kind = sharePreviewFolder
	case h.thumbnails.Supports(info.Name):
		preview.kind = sharePreviewThumbnail
	}
	return preview
}

func sharePageMeta(preview sharePreview, origin, token string) static.PageMeta {
	var defaultImage *static.PageImage
	if origin != "" {
		defaultImage = &static.PageImage{
			URL:    origin + "/" + defaultPreviewImage,
			Width:  defaultPreviewImageWidth,
			Height: defaultPreviewImageHeight,
		}
	}
	if preview.kind == sharePreviewGeneric {
		return static.PageMeta{
			Title:       "Shared - BoxBox",
			OGTitle:     "BoxBox",
			Description: "Self-hosted file sharing",
			Image:       defaultImage,
			Card:        static.TwitterCardSummary,
		}
	}

	name := previewText(preview.name)
	meta := static.PageMeta{
		// Isolate the name so right-to-left text cannot reorder the suffix.
		Title:       "\u2068" + name + "\u2069 - BoxBox",
		OGTitle:     name,
		Description: "View this shared file on BoxBox",
		Image:       defaultImage,
		Card:        static.TwitterCardSummary,
	}
	if origin != "" {
		meta.URL = origin + "/s/" + url.PathEscape(token)
	}
	switch preview.kind {
	case sharePreviewFolder:
		meta.Description = "View this shared folder on BoxBox"
	case sharePreviewThumbnail:
		if origin != "" {
			meta.Image = &static.PageImage{URL: origin + "/api/v1/share/" + url.PathEscape(token) +
				"/thumbnail?v=" + strconv.FormatInt(preview.modTime, 10)}
			meta.Card = static.TwitterCardLargeImage
		}
	}
	return meta
}

// previewText makes a file name safe to show as one line: valid UTF-8, no control or bidi
// formatting characters, and bounded length.
func previewText(s string) string {
	s = strings.ToValidUTF8(s, "\uFFFD")
	s = strings.Map(func(r rune) rune {
		switch {
		case unicode.IsControl(r):
			return ' '
		case unicode.Is(unicode.Bidi_Control, r):
			return -1
		}
		return r
	}, s)
	if utf8.RuneCountInString(s) > maxPreviewNameRunes {
		s = string([]rune(s)[:maxPreviewNameRunes-1]) + "\u2026"
	}
	return s
}

// origin returns the external origin for absolute preview URLs, or "" when it cannot be trusted to parse.
func (h *SharePreviewHandler) origin(r *http.Request) string {
	if h.publicURL != "" {
		return h.publicURL
	}
	if !validRequestHost(r.Host) {
		return ""
	}
	scheme := "http"
	if requestIsHTTPS(r) {
		scheme = "https"
	}
	return scheme + "://" + r.Host
}

func validRequestHost(host string) bool {
	if host == "" || len(host) > 255 {
		return false
	}
	u, err := url.Parse("//" + host)
	if err != nil || u.Host != host || u.User != nil || u.Path != "" {
		return false
	}
	if port := u.Port(); port != "" {
		if n, err := strconv.Atoi(port); err != nil || n < 1 || n > 65535 {
			return false
		}
	}
	hostname := u.Hostname()
	if _, err := netip.ParseAddr(hostname); err == nil {
		return true
	}
	if hostname == "" || strings.Contains(host, "[") {
		return false
	}
	for _, r := range hostname {
		if !(r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' || r >= '0' && r <= '9' || r == '.' || r == '-') {
			return false
		}
	}
	return true
}

// Image serves the og:image for a single-file share: a thumbnail, or the default image when the
// file cannot be rendered. Crawlers drop cards whose image URL redirects or is not an image.
// GET /api/v1/share/{token}/thumbnail
func (h *SharePreviewHandler) Image(w http.ResponseWriter, r *http.Request) {
	ifNoneMatch := r.Header.Get("If-None-Match")
	thumbnail, err := h.thumbnails.Render(r.Context(), chi.URLParam(r, "token"),
		sharePreviewImageSize, sharePreviewImageSize, ifNoneMatch)
	w.Header().Set("Cache-Control", "no-cache")
	switch {
	case err == nil:
		w.Header().Set("Content-Type", "image/jpeg")
		w.Header().Set("Content-Security-Policy", streamSandboxCSP)
		w.Header().Set("ETag", thumbnail.ETag)
		w.Header().Set("Content-Length", strconv.Itoa(len(thumbnail.Content)))
		_, _ = w.Write(thumbnail.Content)
	case errors.Is(err, service.ErrThumbnailNotModified):
		w.Header().Set("ETag", ifNoneMatch)
		w.WriteHeader(http.StatusNotModified)
	case h.defaultImage != nil && (errors.Is(err, service.ErrThumbnailUnsupported) ||
		errors.Is(err, service.ErrThumbnailTooLarge) || errors.Is(err, service.ErrThumbnailUnavailable) ||
		errors.Is(err, service.ErrNotFile)):
		w.Header().Set("ETag", h.defaultETag)
		if ifNoneMatch == h.defaultETag {
			w.WriteHeader(http.StatusNotModified)
			return
		}
		w.Header().Set("Content-Type", "image/png")
		w.Header().Set("Content-Length", strconv.Itoa(len(h.defaultImage)))
		_, _ = w.Write(h.defaultImage)
	default:
		HandleServiceError(w, err)
	}
}
