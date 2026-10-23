package service

import (
	"context"
	"io"
	"mime"
	"net/http"
	"net/url"
	"strings"
	"time"
	"unicode"
)

func safeGatewayPath(value string) bool {
	if !strings.HasPrefix(value, "/") || strings.ContainsAny(value, "\\") {
		return false
	}
	for _, r := range value {
		if unicode.IsControl(r) {
			return false
		}
	}
	for _, part := range strings.Split(strings.TrimPrefix(value, "/"), "/") {
		if part == "." || part == ".." || part == "" {
			return false
		}
	}
	// Reject another proxy's second decoding of separators or dot segments.
	lowered := strings.ToLower(value)
	for _, escape := range []string{"%2f", "%5c", "%2e", "%00"} {
		if strings.Contains(lowered, escape) {
			return false
		}
	}
	return true
}

// DesktopGatewayAllowed matches actual API route shapes and methods.
func DesktopGatewayAllowed(method, route string) bool {
	route = strings.TrimSuffix(route, "/")
	if !safeGatewayPath(route) {
		return false
	}
	p := strings.Split(strings.TrimPrefix(route, "/"), "/")
	switch p[0] {
	case "files":
		if len(p) == 1 || len(p) == 2 && p[1] == "stats" {
			return method == http.MethodGet
		}
		if p[1] == "list" {
			return len(p) > 2 && method == http.MethodGet
		}
		return method == "GET" || method == "POST" || method == "PUT" || method == "PATCH" || method == "DELETE"
	case "stream":
		if len(p) < 3 {
			return false
		}
		switch p[1] {
		case "download", "archive", "preview", "thumbnail":
			return method == "GET"
		case "upload":
			if p[2] == "status" {
				return method == "GET"
			}
			return method == "POST"
		}
	case "search":
		return len(p) == 1 && method == "GET"
	case "jobs":
		return len(p) == 1 && (method == "GET" || method == "POST") || len(p) == 2 && (method == "GET" || method == "DELETE")
	case "shares":
		return len(p) == 1 && (method == "GET" || method == "POST") || len(p) == 2 && (method == "PATCH" || method == "DELETE")
	case "system":
		return len(p) == 2 && p[1] == "upload" && method == "GET"
	}
	return false
}

var gatewayRequestHeaders = []string{"Content-Type", "Range", "If-Range", "If-None-Match", "If-Modified-Since", "If-Unmodified-Since", "If-Match", "X-Upload-ID", "X-Chunk-Index", "X-Total-Chunks", "X-Chunk-Size", "X-Total-Size", "X-Checksum", "X-Create-Only", "X-Upload-Type"}

func gatewayJSON(headers http.Header) bool {
	value, _, err := mime.ParseMediaType(headers.Get("Content-Type"))
	return err == nil && value == "application/json"
}

func gatewayQueryAllowed(query url.Values) bool {
	for name := range query {
		switch name {
		case "page", "pageSize", "includeHidden", "sortBy", "sortDir", "filter", "path", "q", "confirm", "width", "height", "uploadId":
		default:
			return false
		}
	}
	return true
}

func (s *DesktopServers) request(ctx context.Context, record desktopServerRecord, method, route string, query url.Values, headers http.Header, body io.Reader) (*http.Response, error) {
	u, err := desktopServerURL(record.URL)
	if err != nil {
		return nil, ErrRemoteUnavailable
	}
	u.Path += "/api/v1" + route
	u.RawQuery = query.Encode()
	req, err := http.NewRequestWithContext(ctx, method, u.String(), body)
	if err != nil {
		return nil, ErrRemoteUnavailable
	}
	for _, name := range gatewayRequestHeaders {
		for _, value := range headers.Values(name) {
			req.Header.Add(name, value)
		}
	}
	req.Header.Set("Authorization", "Bearer "+record.Credential)
	req.Header.Set("Accept-Encoding", "identity")
	response, err := s.client.Do(req)
	if err != nil {
		return nil, ErrRemoteUnavailable
	}
	if response.StatusCode >= 300 && response.StatusCode < 400 && response.StatusCode != http.StatusNotModified {
		response.Body.Close()
		return nil, ErrRemoteUnavailable
	}
	if response.StatusCode == 401 || response.StatusCode == 403 {
		response.Body.Close()
		return nil, ErrRemoteAuthRequired
	}
	return response, nil
}

func (s *DesktopServers) Forward(ctx context.Context, id, method, route string, query url.Values, headers http.Header, body io.Reader) (*http.Response, error) {
	if !DesktopGatewayAllowed(method, route) || !gatewayQueryAllowed(query) {
		return nil, ErrRemoteRoute
	}
	record, err := s.record(id)
	if err != nil {
		return nil, err
	}
	if record.Credential == "" || !record.ExpiresAt.After(time.Now()) {
		return nil, ErrRemoteAuthRequired
	}
	response, err := s.request(ctx, record, method, route, query, headers, body)
	if err != nil {
		return nil, err
	}
	streaming := method == "GET" && (strings.HasPrefix(route, "/stream/download/") || strings.HasPrefix(route, "/stream/archive/") || strings.HasPrefix(route, "/stream/preview/") || strings.HasPrefix(route, "/stream/thumbnail/"))
	if response.StatusCode != 204 && response.StatusCode != 304 && !streaming && !gatewayJSON(response.Header) {
		response.Body.Close()
		return nil, ErrRemoteUnavailable
	}
	return response, nil
}
