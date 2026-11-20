package config

import (
	"encoding/gob"
	"fmt"
	"io"
)

// Encode writes the loaded configuration for a child process. In NAS mode the
// supervisor reads config.yaml as root and hands the result to the API, which
// runs unprivileged and need not be able to read the file.
func (r *LoadResult) Encode(w io.Writer) error {
	return gob.NewEncoder(w).Encode(r)
}

// Decode reads a configuration written by Encode.
func Decode(r io.Reader) (*LoadResult, error) {
	var result LoadResult
	if err := gob.NewDecoder(r).Decode(&result); err != nil {
		return nil, fmt.Errorf("read configuration from the NAS supervisor: %w", err)
	}
	if result.Config == nil {
		return nil, fmt.Errorf("read configuration from the NAS supervisor: no configuration")
	}
	return &result, nil
}
