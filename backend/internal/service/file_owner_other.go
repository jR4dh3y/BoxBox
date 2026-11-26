//go:build !linux

package service

import (
	"io/fs"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

// ownership has nothing to add where a stat carries no owner.
func ownership(fs.FileInfo, *model.ItemProperties) {}
