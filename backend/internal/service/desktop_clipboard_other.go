//go:build !linux

package service

import (
	"context"
	"fmt"
)

type unavailableDesktopClipboardAdapter struct{}

func newLinuxClipboardAdapter() clipboardAdapter {
	return unavailableDesktopClipboardAdapter{}
}

func (unavailableDesktopClipboardAdapter) Read(context.Context) (map[string][]byte, error) {
	return nil, unavailableDesktopClipboardError()
}

func (unavailableDesktopClipboardAdapter) Write(context.Context, DesktopClipboardOperation, map[string][]byte) error {
	return unavailableDesktopClipboardError()
}

func (unavailableDesktopClipboardAdapter) Clear(context.Context) error {
	return unavailableDesktopClipboardError()
}

func unavailableDesktopClipboardError() error {
	return fmt.Errorf("%w: native clipboard integration is supported on Linux only", ErrDesktopClipboardUnavailable)
}
