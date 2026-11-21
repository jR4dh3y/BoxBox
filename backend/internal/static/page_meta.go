package static

import (
	"bytes"
	"html"
	"net/http"
	"strconv"
)

// TwitterCard is the twitter:card layout.
type TwitterCard string

const (
	TwitterCardSummary    TwitterCard = "summary"
	TwitterCardLargeImage TwitterCard = "summary_large_image"
)

// PageImage is an absolute image URL; zero Width or Height is omitted.
type PageImage struct {
	URL    string
	Width  int
	Height int
}

// PageMeta is plain text for link-preview crawlers. ServeIndexWithMeta escapes every field.
type PageMeta struct {
	Title       string
	OGTitle     string
	Description string
	URL         string     // absolute; "" omits og:url
	Image       *PageImage // nil omits og:image
	Card        TwitterCard
}

// ServeIndexWithMeta serves the SPA document with meta tags inserted before </head>.
func (h *Handler) ServeIndexWithMeta(w http.ResponseWriter, r *http.Request, meta PageMeta) {
	body := h.indexHTML
	if h.indexHead != nil {
		var buf bytes.Buffer
		buf.Grow(len(h.indexHTML) + 1024)
		buf.Write(h.indexHead)
		writePageMeta(&buf, meta)
		buf.Write(h.indexTail)
		body = buf.Bytes()
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Header().Set("Cache-Control", "no-cache")
	w.Header().Set("Content-Length", strconv.Itoa(len(body)))
	if r.Method != http.MethodHead {
		_, _ = w.Write(body)
	}
}

func writePageMeta(buf *bytes.Buffer, meta PageMeta) {
	buf.WriteString("<title>")
	buf.WriteString(html.EscapeString(meta.Title))
	buf.WriteString("</title>")
	writeMetaTag(buf, "property", "og:type", "website")
	writeMetaTag(buf, "property", "og:site_name", "BoxBox")
	writeMetaTag(buf, "property", "og:title", meta.OGTitle)
	writeMetaTag(buf, "property", "og:description", meta.Description)
	writeMetaTag(buf, "name", "description", meta.Description)
	if meta.URL != "" {
		writeMetaTag(buf, "property", "og:url", meta.URL)
	}
	if meta.Image != nil {
		writeMetaTag(buf, "property", "og:image", meta.Image.URL)
		if meta.Image.Width > 0 && meta.Image.Height > 0 {
			writeMetaTag(buf, "property", "og:image:width", strconv.Itoa(meta.Image.Width))
			writeMetaTag(buf, "property", "og:image:height", strconv.Itoa(meta.Image.Height))
		}
	}
	writeMetaTag(buf, "name", "twitter:card", string(meta.Card))
}

func writeMetaTag(buf *bytes.Buffer, attr, key, content string) {
	buf.WriteString(`<meta `)
	buf.WriteString(attr)
	buf.WriteString(`="`)
	buf.WriteString(key)
	buf.WriteString(`" content="`)
	buf.WriteString(html.EscapeString(content))
	buf.WriteString(`">`)
}
