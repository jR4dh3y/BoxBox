package middleware

import "testing"

func TestAcceptsGzip(t *testing.T) {
	cases := []struct {
		header string
		want   bool
	}{
		{"", false},
		{"gzip", true},
		{"GZIP", true},
		{"br, gzip", true},
		{"gzip;q=1", true},
		{"gzip; q=0.5", true},
		{"gzip;Q=0.001", true},
		{"gzip;q=0", false},
		{"gzip;q=0.0", false},
		{"gzip;q=0.000", false},
		{"gzip; q=0", false},
		{"br, gzip;q=0.0, deflate", false},
		{"gzip;q=abc", false},
		{"deflate, br", false},
	}
	for _, tc := range cases {
		if got := acceptsGzip(tc.header); got != tc.want {
			t.Errorf("acceptsGzip(%q) = %v, want %v", tc.header, got, tc.want)
		}
	}
}
