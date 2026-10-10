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

func TestCompareFoldMatchesToLowerCompare(t *testing.T) {
	names := []string{"", "a", "A", "b", "B", "ab", "Ab", "aB", "a.txt", "a-b", "Zed", "zed.md", "10.txt", "9.txt", "éclair", "Éclair", "eclair", "Éclair2", "aé", "aÉ", "İ", "i"}
	for _, a := range names {
		for _, b := range names {
			want := strings.Compare(strings.ToLower(a), strings.ToLower(b))
			if got := compareFold(a, b); got != want {
				t.Errorf("compareFold(%q, %q) = %d, want %d", a, b, got, want)
			}
		}
	}
}
