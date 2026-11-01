package service

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

type ChooserRule struct {
	Kind  uint32 `json:"kind"`
	Value string `json:"value"`
}
type ChooserFilter struct {
	Name  string        `json:"name"`
	Rules []ChooserRule `json:"rules"`
}
type ChooserOption struct {
	ID    string `json:"id"`
	Label string `json:"label"`
}
type ChooserChoice struct {
	ID      string          `json:"id"`
	Label   string          `json:"label"`
	Options []ChooserOption `json:"options"`
	Value   string          `json:"value"`
}
type ChooserRequest struct {
	ID          string          `json:"id"`
	Kind        string          `json:"kind"`
	AppID       string          `json:"appId"`
	Title       string          `json:"title"`
	AcceptLabel string          `json:"acceptLabel"`
	Multiple    bool            `json:"multiple"`
	Directory   bool            `json:"directory"`
	Folder      string          `json:"folder"`
	Name        string          `json:"name"`
	Files       []string        `json:"files"`
	Filters     []ChooserFilter `json:"filters"`
	Filter      int             `json:"filter"`
	Choices     []ChooserChoice `json:"choices"`
}
type ChooserSelection struct {
	Paths   []string          `json:"paths"`
	Filter  int               `json:"filter"`
	Choices map[string]string `json:"choices"`
	Replace bool              `json:"replace"`
	Cancel  bool              `json:"cancel"`
}
type ChooserChoiceResult struct {
	ID    string
	Value string
}

type ChooserResult struct {
	Code    uint32
	URIs    []string
	Filter  *ChooserFilter
	Choices []ChooserChoiceResult
}
type chooserPending struct {
	request ChooserRequest
	result  chan ChooserResult
}

var ErrChooserClosed = errors.New("file chooser request is closed")
var ErrChooserReplace = errors.New("a destination already exists; confirm replacement")

// DesktopChooser owns pending local portal interactions; every returned path is mount-resolved.
type DesktopChooser struct {
	mu      sync.Mutex
	pending map[string]*chooserPending
	changed chan struct{}
	mounts  *mounts
	fs      filesystem.FS
}

func NewDesktopChooser(fsys filesystem.FS, points []model.MountPoint) *DesktopChooser {
	return &DesktopChooser{pending: make(map[string]*chooserPending), changed: make(chan struct{}), mounts: newMounts(fsys, fixedMounts(points)), fs: fsys}
}
func (c *DesktopChooser) notify() { close(c.changed); c.changed = make(chan struct{}) }
func (c *DesktopChooser) Begin(request ChooserRequest) (ChooserRequest, <-chan ChooserResult) {
	c.mu.Lock()
	defer c.mu.Unlock()
	key := make([]byte, 16)
	_, _ = rand.Read(key)
	request.ID = hex.EncodeToString(key)
	p := &chooserPending{request: request, result: make(chan ChooserResult, 1)}
	c.pending[request.ID] = p
	c.notify()
	return request, p.result
}
func (c *DesktopChooser) List() ([]ChooserRequest, <-chan struct{}) {
	c.mu.Lock()
	defer c.mu.Unlock()
	list := make([]ChooserRequest, 0, len(c.pending))
	for _, p := range c.pending {
		list = append(list, p.request)
	}
	return list, c.changed
}
func (c *DesktopChooser) Get(id string) (ChooserRequest, error) {
	c.mu.Lock()
	defer c.mu.Unlock()
	if p := c.pending[id]; p != nil {
		return p.request, nil
	}
	return ChooserRequest{}, ErrChooserClosed
}
func (c *DesktopChooser) Cancel(id string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	if p := c.pending[id]; p != nil {
		delete(c.pending, id)
		p.result <- ChooserResult{Code: 1}
		c.notify()
	}
}
func (c *DesktopChooser) Close() {
	list, _ := c.List()
	for _, r := range list {
		c.Cancel(r.ID)
	}
}
func (c *DesktopChooser) Select(ctx context.Context, id string, selection ChooserSelection) error {
	if selection.Cancel {
		c.Cancel(id)
		return nil
	}
	request, err := c.Get(id)
	if err != nil {
		return err
	}
	if len(selection.Paths) == 0 || len(selection.Paths) > 1024 || (!request.Multiple && len(selection.Paths) != 1) {
		return fmt.Errorf("%w: invalid selection count", ErrInvalidOperation)
	}
	paths := selection.Paths
	if request.Kind == "save-files" {
		if len(paths) != 1 {
			return fmt.Errorf("%w: select one destination folder", ErrInvalidOperation)
		}
		paths = make([]string, 0, len(request.Files))
		for _, name := range request.Files {
			paths = append(paths, selection.Paths[0]+"/"+name)
		}
	}
	result := ChooserResult{URIs: make([]string, 0, len(paths)), Choices: make([]ChooserChoiceResult, 0, len(request.Choices))}
	if len(request.Filters) > 0 {
		if selection.Filter < 0 || selection.Filter >= len(request.Filters) {
			return fmt.Errorf("%w: invalid file filter", ErrInvalidOperation)
		}
		result.Filter = &request.Filters[selection.Filter]
	}
	for _, choice := range request.Choices {
		value := selection.Choices[choice.ID]
		valid := false
		if len(choice.Options) == 0 {
			valid = value == "true" || value == "false"
		} else {
			for _, option := range choice.Options {
				if option.ID == value {
					valid = true
				}
			}
		}
		if !valid {
			return fmt.Errorf("%w: invalid choice", ErrInvalidOperation)
		}
		result.Choices = append(result.Choices, ChooserChoiceResult{ID: choice.ID, Value: value})
	}
	for _, path := range paths {
		if err := ctx.Err(); err != nil {
			return err
		}
		access := readExisting
		if request.Kind != "open" {
			access = writeMaybeMissing
		}
		_, resolved, err := c.mounts.resolve(path, access)
		if err != nil {
			return err
		}
		info, statErr := c.fs.Stat(resolved)
		if request.Kind == "open" {
			if statErr != nil {
				return statErr
			}
			if info.IsDir() != request.Directory {
				return fmt.Errorf("%w: select the requested file type", ErrInvalidOperation)
			}
			if !info.IsDir() && result.Filter != nil && !chooserMatches(*result.Filter, info.Name(), resolved) {
				return fmt.Errorf("%w: file does not match the selected filter", ErrInvalidOperation)
			}
		} else {
			if statErr == nil && info.IsDir() {
				return ErrNotFile
			}
			if statErr == nil && !selection.Replace {
				return ErrChooserReplace
			}
			if statErr != nil && !errors.Is(statErr, os.ErrNotExist) {
				return statErr
			}
		}
		result.URIs = append(result.URIs, (&url.URL{Scheme: "file", Path: resolved}).String())
	}
	c.mu.Lock()
	defer c.mu.Unlock()
	p := c.pending[id]
	if p == nil {
		return ErrChooserClosed
	}
	delete(c.pending, id)
	p.result <- result
	c.notify()
	return nil
}

// Initial local suggestions are hints, never grants; acceptance resolves them again.
func (c *DesktopChooser) Folder(local string) string {
	if !filepath.IsAbs(local) {
		local, _ = os.UserHomeDir()
	}
	for _, point := range c.mounts.list() {
		relative, err := filepath.Rel(point.Path, local)
		if err != nil || relative == ".." || strings.HasPrefix(relative, "../") {
			continue
		}
		virtual := point.Name
		if relative != "." {
			virtual += "/" + filepath.ToSlash(relative)
		}
		_, resolved, err := c.mounts.resolve(virtual, readExisting)
		if err != nil {
			continue
		}
		info, err := c.fs.Stat(resolved)
		if err == nil && info.IsDir() {
			return virtual
		}
	}
	return ""
}
