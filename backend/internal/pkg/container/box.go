// Package container provides generic container types with method type parameters (Go 1.27+).
package container

import (
	"fmt"
	"iter"
)

// Box holds a single generic value and provides monadic transformation methods.
type Box[T any] struct {
	value T
}

// NewBox creates a new Box containing the specified value.
func NewBox[T any](value T) Box[T] {
	return Box[T]{value: value}
}

// Value returns the wrapped value.
func (box Box[T]) Value() T {
	return box.value
}

// Map applies a transformation function to the wrapped value and returns a new Box[U].
// This leverages Go 1.27 method type parameters.
func (box Box[T]) Map[U any](transform func(T) U) Box[U] {
	return Box[U]{
		value: transform(box.value),
	}
}

// FlatMap applies a transformation function returning a Box[U] and flattens the result.
func (box Box[T]) FlatMap[U any](transform func(T) Box[U]) Box[U] {
	return transform(box.value)
}

// String returns the string representation of the Box.
func (box Box[T]) String() string {
	return fmt.Sprintf("Box(%v)", box.value)
}

// Result represents either a successful computation containing a value T, or an error.
type Result[T any] struct {
	value T
	err   error
}

// Ok creates a successful Result.
func Ok[T any](value T) Result[T] {
	return Result[T]{value: value, err: nil}
}

// Err creates a failed Result with the provided error.
func Err[T any](err error) Result[T] {
	return Result[T]{err: err}
}

// IsOk returns true if the Result represents a success.
func (r Result[T]) IsOk() bool {
	return r.err == nil
}

// IsErr returns true if the Result represents a failure.
func (r Result[T]) IsErr() bool {
	return r.err != nil
}

// Value returns the wrapped value, which is only valid if IsOk() is true.
func (r Result[T]) Value() T {
	return r.value
}

// Error returns the wrapped error, or nil if successful.
func (r Result[T]) Error() error {
	return r.err
}

// Unwrap returns both the value and error.
func (r Result[T]) Unwrap() (T, error) {
	return r.value, r.err
}

// OrElse returns the wrapped value if successful, or defaultVal if failed.
func (r Result[T]) OrElse(defaultVal T) T {
	if r.err != nil {
		return defaultVal
	}
	return r.value
}

// Map applies a transformation function to the wrapped value if successful.
// If the Result is an error, it propagates the error to Result[U].
func (r Result[T]) Map[U any](transform func(T) U) Result[U] {
	if r.err != nil {
		return Result[U]{err: r.err}
	}
	return Result[U]{value: transform(r.value)}
}

// FlatMap applies a transformation returning Result[U] if successful.
func (r Result[T]) FlatMap[U any](transform func(T) Result[U]) Result[U] {
	if r.err != nil {
		return Result[U]{err: r.err}
	}
	return transform(r.value)
}

// Slice wraps a standard slice to provide fluent, generic functional transformations.
type Slice[T any] []T

// NewSlice creates a Slice from individual elements.
func NewSlice[T any](items ...T) Slice[T] {
	return Slice[T](items)
}

// Len returns the number of items.
func (s Slice[T]) Len() int {
	return len(s)
}

// ToSlice converts back to a standard Go slice []T.
func (s Slice[T]) ToSlice() []T {
	return []T(s)
}

// Map transforms each element using the provided transform function.
func (s Slice[T]) Map[U any](transform func(T) U) Slice[U] {
	result := make(Slice[U], len(s))
	for i, item := range s {
		result[i] = transform(item)
	}
	return result
}

// Filter keeps only elements satisfying the predicate function.
func (s Slice[T]) Filter(predicate func(T) bool) Slice[T] {
	result := make(Slice[T], 0, len(s))
	for _, item := range s {
		if predicate(item) {
			result = append(result, item)
		}
	}
	return result
}

// FlatMap transforms each element to a slice of U and flattens the result.
func (s Slice[T]) FlatMap[U any](transform func(T) []U) Slice[U] {
	result := make(Slice[U], 0, len(s))
	for _, item := range s {
		result = append(result, transform(item)...)
	}
	return result
}

// Reduce aggregates elements into an accumulator value.
func (s Slice[T]) Reduce[U any](initial U, accumulator func(U, T) U) U {
	curr := initial
	for _, item := range s {
		curr = accumulator(curr, item)
	}
	return curr
}

// All returns an iterator over index-value pairs (Go 1.23+ iter.Seq2).
func (s Slice[T]) All() iter.Seq2[int, T] {
	return func(yield func(int, T) bool) {
		for i, v := range s {
			if !yield(i, v) {
				return
			}
		}
	}
}

// Values returns an iterator over values (Go 1.23+ iter.Seq).
func (s Slice[T]) Values() iter.Seq[T] {
	return func(yield func(T) bool) {
		for _, v := range s {
			if !yield(v) {
				return
			}
		}
	}
}

// PagedList represents a paginated generic collection with method type parameter transformations.
type PagedList[T any] struct {
	Items      Slice[T] `json:"items"`
	TotalCount int      `json:"totalCount"`
	Page       int      `json:"page"`
	PageSize   int      `json:"pageSize"`
}

// NewPagedList creates a new PagedList.
func NewPagedList[T any](items []T, totalCount, page, pageSize int) PagedList[T] {
	return PagedList[T]{
		Items:      NewSlice(items...),
		TotalCount: totalCount,
		Page:       page,
		PageSize:   pageSize,
	}
}

// Map transforms each item in the PagedList using the transform function.
// This leverages Go 1.27 method type parameters.
func (p PagedList[T]) Map[U any](transform func(T) U) PagedList[U] {
	return PagedList[U]{
		Items:      p.Items.Map(transform),
		TotalCount: p.TotalCount,
		Page:       p.Page,
		PageSize:   p.PageSize,
	}
}
