package service

import (
	"strings"
	"testing"
)

func TestContainsFoldMatchesToLowerContains(t *testing.T) {
	names := []string{"", "Report-0001.TXT", "photo.JPG", "İstanbul.txt", "Kelvin-\u212A.txt", "naïve CAFÉ.md", "aaa", "a"}
	queries := []string{"", "report", "txt", "jpg", "i̇stanbul", "k", "café", "aa", "aaaa", "zzz", "é"}
	for _, name := range names {
		for _, query := range queries {
			want := strings.Contains(strings.ToLower(name), query)
			if got := containsFold(name, query); got != want {
				t.Errorf("containsFold(%q, %q) = %v, want %v", name, query, got, want)
			}
		}
	}
}
