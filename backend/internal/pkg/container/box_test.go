package container_test

import (
	"errors"
	"fmt"
	"slices"
	"strconv"
	"testing"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/container"
)

func TestBoxMapAndFlatMap(t *testing.T) {
	b := container.NewBox(42)
	if b.Value() != 42 {
		t.Fatalf("expected 42, got %d", b.Value())
	}

	// Go 1.27 method type parameter Map[U]
	strBox := b.Map(func(x int) string {
		return fmt.Sprintf("val:%d", x)
	})
	if strBox.Value() != "val:42" {
		t.Fatalf("expected 'val:42', got '%s'", strBox.Value())
	}

	// FlatMap
	doubled := b.FlatMap(func(x int) container.Box[int] {
		return container.NewBox(x * 2)
	})
	if doubled.Value() != 84 {
		t.Fatalf("expected 84, got %d", doubled.Value())
	}
}

func TestResultMapAndFlatMap(t *testing.T) {
	okRes := container.Ok(100)
	if !okRes.IsOk() || okRes.IsErr() {
		t.Fatal("expected okRes to be Ok")
	}

	mapped := okRes.Map(func(x int) string {
		return strconv.Itoa(x)
	})
	if mapped.Value() != "100" {
		t.Fatalf("expected '100', got '%s'", mapped.Value())
	}

	errRes := container.Err[int](errors.New("test error"))
	if !errRes.IsErr() || errRes.IsOk() {
		t.Fatal("expected errRes to be Err")
	}
	errMapped := errRes.Map(func(x int) string {
		return strconv.Itoa(x)
	})
	if !errMapped.IsErr() {
		t.Fatal("expected errMapped to remain Err")
	}
	if errMapped.Error().Error() != "test error" {
		t.Fatalf("expected 'test error', got '%v'", errMapped.Error())
	}
}

func TestSliceFunctionalMethodsAndIterators(t *testing.T) {
	s := container.NewSlice(1, 2, 3, 4, 5)

	// Map
	doubled := s.Map(func(x int) int { return x * 2 })
	if !slices.Equal(doubled.ToSlice(), []int{2, 4, 6, 8, 10}) {
		t.Fatalf("unexpected doubled slice: %v", doubled.ToSlice())
	}

	// Filter
	evens := s.Filter(func(x int) bool { return x%2 == 0 })
	if !slices.Equal(evens.ToSlice(), []int{2, 4}) {
		t.Fatalf("unexpected evens slice: %v", evens.ToSlice())
	}

	// Reduce
	sum := s.Reduce(0, func(acc, item int) int { return acc + item })
	if sum != 15 {
		t.Fatalf("expected sum 15, got %d", sum)
	}

	// Range-over-func iterator (Go 1.23+)
	var collected []int
	for _, v := range s.All() {
		collected = append(collected, v)
	}
	if !slices.Equal(collected, []int{1, 2, 3, 4, 5}) {
		t.Fatalf("unexpected collected values from All: %v", collected)
	}
}

func TestPagedListMap(t *testing.T) {
	items := []int{10, 20, 30}
	paged := container.NewPagedList(items, 100, 1, 3)

	mapped := paged.Map(func(x int) string {
		return fmt.Sprintf("$%d", x)
	})

	if mapped.TotalCount != 100 || mapped.Page != 1 || mapped.PageSize != 3 {
		t.Fatalf("metadata corrupted after Map: %+v", mapped)
	}
	if !slices.Equal(mapped.Items.ToSlice(), []string{"$10", "$20", "$30"}) {
		t.Fatalf("unexpected mapped items: %v", mapped.Items.ToSlice())
	}
}
