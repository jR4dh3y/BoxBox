//go:build linux

package service

import (
	"io/fs"
	"os/user"
	"strconv"
	"sync"
	"syscall"
	"time"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

var (
	userNames  sync.Map
	groupNames sync.Map
)

// lookupName returns the account's name, or its number when the system has no name for it.
func lookupName(cache *sync.Map, id uint32, lookup func(string) (string, error)) string {
	if name, ok := cache.Load(id); ok {
		return name.(string)
	}
	key := strconv.FormatUint(uint64(id), 10)
	name, err := lookup(key)
	if err != nil || name == "" {
		name = key
	}
	cache.Store(id, name)
	return name
}

// ownership fills the owner, group and the access and change times from a local stat.
func ownership(info fs.FileInfo, props *model.ItemProperties) {
	stat, ok := info.Sys().(*syscall.Stat_t)
	if !ok {
		return
	}
	props.Owner = lookupName(&userNames, stat.Uid, func(id string) (string, error) {
		account, err := user.LookupId(id)
		if err != nil {
			return "", err
		}
		return account.Username, nil
	})
	props.Group = lookupName(&groupNames, stat.Gid, func(id string) (string, error) {
		group, err := user.LookupGroupId(id)
		if err != nil {
			return "", err
		}
		return group.Name, nil
	})
	accessed := time.Unix(int64(stat.Atim.Sec), int64(stat.Atim.Nsec)).UTC()
	changed := time.Unix(int64(stat.Ctim.Sec), int64(stat.Ctim.Nsec)).UTC()
	props.Accessed, props.Changed = &accessed, &changed
}
