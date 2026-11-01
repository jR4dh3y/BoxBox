//go:build !linux

package service

import "errors"

func readVolumeSpace(path string) (volumeSpace, error) {
	return volumeSpace{}, errors.New("removable volume space is only supported on Linux")
}
