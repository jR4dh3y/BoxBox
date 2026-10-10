package service

import "strings"

// containsFold reports whether name contains lowerQuery, ignoring case.
// lowerQuery must already be lower-cased. ASCII names, the common case, are compared without allocating.
func containsFold(name, lowerQuery string) bool {
	if lowerQuery == "" {
		return true
	}
	if !isASCII(name) || !isASCII(lowerQuery) {
		return strings.Contains(strings.ToLower(name), lowerQuery)
	}
	for start := 0; start+len(lowerQuery) <= len(name); start++ {
		i := 0
		for ; i < len(lowerQuery); i++ {
			c := name[start+i]
			if 'A' <= c && c <= 'Z' {
				c += 'a' - 'A'
			}
			if c != lowerQuery[i] {
				break
			}
		}
		if i == len(lowerQuery) {
			return true
		}
	}
	return false
}

func isASCII(s string) bool {
	for i := 0; i < len(s); i++ {
		if s[i] >= 0x80 {
			return false
		}
	}
	return true
}

// compareFold orders a and b as strings.Compare(strings.ToLower(a), strings.ToLower(b)) would,
// without allocating when both are ASCII up to the first difference.
func compareFold(a, b string) int {
	n := min(len(a), len(b))
	for i := 0; i < n; i++ {
		ca, cb := a[i], b[i]
		if ca|cb >= 0x80 {
			return strings.Compare(strings.ToLower(a), strings.ToLower(b))
		}
		if 'A' <= ca && ca <= 'Z' {
			ca += 'a' - 'A'
		}
		if 'A' <= cb && cb <= 'Z' {
			cb += 'a' - 'A'
		}
		if ca != cb {
			if ca < cb {
				return -1
			}
			return 1
		}
	}
	switch {
	case len(a) < len(b):
		return -1
	case len(a) > len(b):
		return 1
	default:
		return 0
	}
}
