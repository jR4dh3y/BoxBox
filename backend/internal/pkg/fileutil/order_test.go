package fileutil

import "testing"

func TestCompareNamesNaturalAndDeterministic(t *testing.T) {
	for _, pair := range [][2]string{{"file2.txt", "file10.txt"}, {"file02.txt", "file2.txt"}, {"File.txt", "file.txt"}, {"a999999999999999999999", "a1000000000000000000000"}} {
		if CompareNames(pair[0], pair[1]) >= 0 || CompareNames(pair[1], pair[0]) <= 0 || CompareNames(pair[0], pair[0]) != 0 {
			t.Fatalf("wrong comparison for %v", pair)
		}
	}
}
