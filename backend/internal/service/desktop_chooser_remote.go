package service

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

type chooserRemote struct {
	servers *DesktopServers
	dir     string
}

// ConfigureServers wires the existing private gateway; it performs no network I/O.
func (c *DesktopChooser) ConfigureServers(servers *DesktopServers, dir string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.remote = &chooserRemote{servers: servers, dir: dir}
}

func (r *chooserRemote) get(ctx context.Context, ref transferPath, route string, headers http.Header) (*http.Response, error) {
	response, err := r.servers.Forward(ctx, ref.server, "GET", route+ref.path, nil, headers, nil)
	if err != nil {
		return nil, err
	}
	if response.StatusCode != http.StatusOK && response.StatusCode != http.StatusPartialContent {
		response.Body.Close()
		if response.StatusCode == http.StatusNotFound {
			return nil, ErrPathNotFound
		}
		return nil, ErrRemoteUnavailable
	}
	return response, nil
}

func (c *DesktopChooser) importRemote(ctx context.Context, remote *chooserRemote, reference, dir string, filter *ChooserFilter) (string, error) {
	ref, err := parseTransferPath(reference)
	if err != nil || ref.server == "" {
		return "", ErrInvalidOperation
	}
	// The download endpoint validates mount confinement and only streams regular files.
	response, err := remote.get(ctx, ref, "/stream/download/", nil)
	if err != nil {
		return "", err
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return "", ErrRemoteUnavailable
	}
	if err := os.Mkdir(dir, 0700); err != nil {
		return "", err
	}
	name := filepath.Base(ref.path)
	local := filepath.Join(dir, name)
	file, err := os.OpenFile(local, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
	if err != nil {
		return "", err
	}
	copied, copyErr := io.Copy(file, response.Body)
	closeErr := file.Close()
	if copyErr != nil {
		return "", copyErr
	}
	if closeErr != nil {
		return "", closeErr
	}
	if response.ContentLength >= 0 && copied != response.ContentLength {
		return "", io.ErrUnexpectedEOF
	}
	if err := ctx.Err(); err != nil {
		return "", err
	}
	if filter != nil {
		matched, err := c.matches(*filter, name, &model.MountPoint{Path: dir}, local)
		if err != nil {
			return "", err
		}
		if !matched {
			return "", fmt.Errorf("%w: file does not match the selected filter", ErrInvalidOperation)
		}
	}
	return local, nil
}

func (c *DesktopChooser) remoteDirectory(ctx context.Context, remote *chooserRemote, request ChooserRequest, reference string) (*model.FileList, error) {
	ref, err := parseTransferPath(reference)
	if err != nil || ref.server == "" {
		return nil, ErrInvalidOperation
	}
	response, err := remote.get(ctx, ref, "/files/list/", nil)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	var list model.FileList
	if err := json.NewDecoder(io.LimitReader(response.Body, 32<<20)).Decode(&list); err != nil {
		return nil, ErrRemoteUnavailable
	}
	list.Path = reference
	needsTypes := false
	for _, filter := range request.Filters {
		for _, rule := range filter.Rules {
			needsTypes = needsTypes || rule.Kind == 1
		}
	}
	for index := range list.Items {
		item := &list.Items[index]
		if !validTransferChild(item.Name) || item.Size < 0 {
			return nil, ErrRemoteUnavailable
		}
		item.Path = reference + "/" + item.Name
		if needsTypes && !request.Directory && !item.IsDir && item.MimeType == "" {
			child := ref
			child.path += "/" + item.Name
			content, err := remote.get(ctx, child, "/stream/download/", http.Header{"Range": {"bytes=0-511"}})
			if err != nil {
				return nil, err
			}
			data, readErr := io.ReadAll(io.LimitReader(content.Body, 512))
			content.Body.Close()
			if readErr != nil {
				return nil, ErrRemoteUnavailable
			}
			item.MimeType = strings.Split(http.DetectContentType(data), ";")[0]
		}
	}
	return &list, nil
}
