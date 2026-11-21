package static

import (
	"html"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"
	"testing/fstest"
)

const testIndex = `<!doctype html><html><head><meta charset="utf-8"><script src="/early-fetch.js" async></script><meta http-equiv="content-security-policy" content="script-src 'self'"></head><body><div>app</div></body></html>`

func testPageHandler(index string, embedded bool) *Handler {
	return newHandler(fstest.MapFS{"index.html": {Data: []byte(index)}}, []byte(index), embedded)
}

func serveMeta(t *testing.T, h *Handler, method string, meta PageMeta) *httptest.ResponseRecorder {
	t.Helper()
	rec := httptest.NewRecorder()
	h.ServeIndexWithMeta(rec, httptest.NewRequest(method, "/s/token", nil), meta)
	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d", rec.Code)
	}
	return rec
}

func TestServeIndexWithMetaInsertsTagsBeforeHead(t *testing.T) {
	h := testPageHandler(testIndex, true)
	rec := serveMeta(t, h, http.MethodGet, PageMeta{
		Title: "a.jpg - BoxBox", OGTitle: "a.jpg", Description: "d", URL: "https://x.test/s/t",
		Image: &PageImage{URL: "https://x.test/og.png", Width: 1200, Height: 630}, Card: TwitterCardLargeImage,
	})
	body := rec.Body.String()
	head, tail, ok := strings.Cut(testIndex, "</head>")
	if !ok || !strings.HasPrefix(body, head) || !strings.HasSuffix(body, "</head>"+tail) {
		t.Fatalf("document bytes around the tags changed:\n%s", body)
	}
	for _, want := range []string{
		`<title>a.jpg - BoxBox</title>`,
		`<meta property="og:title" content="a.jpg">`,
		`<meta property="og:url" content="https://x.test/s/t">`,
		`<meta property="og:image" content="https://x.test/og.png">`,
		`<meta property="og:image:width" content="1200">`,
		`<meta name="twitter:card" content="summary_large_image">`,
	} {
		if !strings.Contains(body, want) {
			t.Errorf("missing %s", want)
		}
	}
	if got := rec.Header().Get("Cache-Control"); got != "no-cache" {
		t.Errorf("Cache-Control = %q", got)
	}
	if got := rec.Header().Get("Content-Length"); got != strconv.Itoa(len(body)) {
		t.Errorf("Content-Length = %q, body is %d bytes", got, len(body))
	}
}

func TestServeIndexWithMetaOmitsEmptyURLAndImage(t *testing.T) {
	body := serveMeta(t, testPageHandler(testIndex, true), http.MethodGet, PageMeta{Title: "t", Card: TwitterCardSummary}).Body.String()
	if strings.Contains(body, "og:url") || strings.Contains(body, "og:image") {
		t.Fatalf("empty URL and image must be omitted:\n%s", body)
	}
}

func TestServeIndexWithMetaHeadHasLengthAndNoBody(t *testing.T) {
	h := testPageHandler(testIndex, true)
	meta := PageMeta{Title: "t", Card: TwitterCardSummary}
	get := serveMeta(t, h, http.MethodGet, meta)
	head := serveMeta(t, h, http.MethodHead, meta)
	if head.Body.Len() != 0 {
		t.Fatalf("HEAD wrote %d bytes", head.Body.Len())
	}
	if head.Header().Get("Content-Length") != get.Header().Get("Content-Length") {
		t.Fatalf("HEAD Content-Length %q, GET %q", head.Header().Get("Content-Length"), get.Header().Get("Content-Length"))
	}
}

func TestServeIndexWithMetaLeavesPlaceholderAlone(t *testing.T) {
	placeholder := `<!DOCTYPE html><html><head><title>BoxBox</title></head><body></body></html>`
	body := serveMeta(t, testPageHandler(placeholder, false), http.MethodGet, PageMeta{Title: "x"}).Body.String()
	if body != placeholder {
		t.Fatalf("placeholder changed: %s", body)
	}
}

func TestServeIndexWithMetaEscapesHostileText(t *testing.T) {
	hostile := []string{
		`"><script>alert(1)</script>.jpg`,
		`</title><script>x</script>`,
		`a'b"c&d.png`,
		`x" onload="y`,
	}
	for _, name := range hostile {
		t.Run(name, func(t *testing.T) {
			body := serveMeta(t, testPageHandler(testIndex, true), http.MethodGet, PageMeta{
				Title: name + " - BoxBox", OGTitle: name, Description: name, Card: TwitterCardSummary,
			}).Body.String()
			injected := strings.TrimSuffix(strings.TrimPrefix(body, strings.Split(testIndex, "</head>")[0]), "</head>"+strings.Split(testIndex, "</head>")[1])
			if strings.Contains(injected, "<script") || strings.Count(injected, "<title>") != 1 || strings.Count(injected, "</title>") != 1 {
				t.Fatalf("unescaped markup in injected tags: %s", injected)
			}
			if strings.Contains(injected, `" onload`) || strings.Contains(injected, `"><`+"script") {
				t.Fatalf("attribute injection: %s", injected)
			}
			want := `<meta property="og:title" content="` + html.EscapeString(name) + `">`
			if !strings.Contains(injected, want) {
				t.Fatalf("missing %s in %s", want, injected)
			}
			if got := html.UnescapeString(html.EscapeString(name)); got != name {
				t.Fatalf("escape does not round-trip: %q", got)
			}
		})
	}
}
