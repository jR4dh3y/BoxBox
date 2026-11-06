package fileutil

import "strings"

// CompareNames orders digit runs numerically without integer overflow, then breaks case/zero ties.
func CompareNames(left, right string) int {
	a, b := strings.ToLower(left), strings.ToLower(right)
	for len(a) > 0 && len(b) > 0 {
		if a[0] >= '0' && a[0] <= '9' && b[0] >= '0' && b[0] <= '9' {
			i, j := 0, 0
			for i < len(a) && a[i] >= '0' && a[i] <= '9' {
				i++
			}
			for j < len(b) && b[j] >= '0' && b[j] <= '9' {
				j++
			}
			x, y := strings.TrimLeft(a[:i], "0"), strings.TrimLeft(b[:j], "0")
			if len(x) < len(y) {
				return -1
			}
			if len(x) > len(y) {
				return 1
			}
			if comparison := strings.Compare(x, y); comparison != 0 {
				return comparison
			}
			a, b = a[i:], b[j:]
			continue
		}
		if a[0] < b[0] {
			return -1
		}
		if a[0] > b[0] {
			return 1
		}
		a, b = a[1:], b[1:]
	}
	if comparison := strings.Compare(a, b); comparison != 0 {
		return comparison
	}
	return strings.Compare(left, right)
}
