package service

import (
	"net/http"
	"strings"
	"unicode"
)

func safeClientRoutePath(value string) bool {
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

// ClientRouteAllowed restricts paired credentials to supported file API operations.
func ClientRouteAllowed(method, route string) bool {
	route = strings.TrimSuffix(route, "/")
	if !safeClientRoutePath(route) {
		return false
	}
	p := strings.Split(strings.TrimPrefix(route, "/"), "/")
	switch p[0] {
	case "files":
		if len(p) == 1 {
			return method == http.MethodGet
		}
		if len(p) == 2 && p[1] == "stats" && method == http.MethodGet {
			return true
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
			if method == http.MethodGet {
				return p[2] == "status"
			}
			return method == http.MethodPost
		}
	case "search":
		return len(p) == 1 && method == "GET"
	case "jobs":
		if len(p) == 2 && p[1] == "events" {
			return method == "GET"
		}
		return len(p) == 1 && (method == "GET" || method == "POST") || len(p) == 2 && (method == "GET" || method == "DELETE")
	case "shares":
		return len(p) == 1 && (method == "GET" || method == "POST") || len(p) == 2 && (method == "PATCH" || method == "DELETE")
	case "system":
		return len(p) == 2 && p[1] == "upload" && method == "GET"
	}
	return false
}
