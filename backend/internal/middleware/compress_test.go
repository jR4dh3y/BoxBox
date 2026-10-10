package middleware

import "testing"

func TestAcceptsGzip(t *testing.T) {
	tests := []struct {
		header string
		want   bool
	}{
		{"", false},
		{"gzip", true},
		{"GZIP", true},
		{"deflate, gzip, br", true},
		{"br, deflate", false},
		{"gzip;q=1", true},
		{"gzip;q=0.5", true},
		{"gzip; q=0.001", true},
		{"gzip;q=0", false},
		{"gzip;q=0.0", false},
		{"gzip;q=0.00", false},
		{"gzip;q=0.000", false},
		{"gzip ; q=0", false},
		{"br, gzip;q=0.000", false},
		{"gzip;q=0.000, br", false},
		{"gzip;q=abc", false},
		{"gzip;q=", false},
		{"gzip;level=1", true},
		{"x-gzip", false},
	}
	for _, test := range tests {
		if got := acceptsGzip(test.header); got != test.want {
			t.Errorf("acceptsGzip(%q) = %v, want %v", test.header, got, test.want)
		}
	}
}
