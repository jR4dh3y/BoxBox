package service

import "testing"

func TestUnescapeMountPath(t *testing.T) {
	tests := map[string]string{
		"/srv/plain":            "/srv/plain",
		`/srv/my\040files`:      "/srv/my files",
		`/srv/tab\011and\134bs`: "/srv/tab\tand\\bs",
		`/srv/trailing\04`:      `/srv/trailing\04`,
	}
	for in, want := range tests {
		if got := unescapeMountPath(in); got != want {
			t.Errorf("unescapeMountPath(%q) = %q, want %q", in, got, want)
		}
	}
}
