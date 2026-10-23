package service

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"uuid"

	"github.com/jR4dh3y/BoxBox/backend/internal/model"
)

const transferChunkSize = 1 << 20

func (s *DesktopTransfers) remote(ctx context.Context, ref transferPath, method, route string, query url.Values, headers http.Header, body io.Reader) (*http.Response, error) {
	response, err := s.servers.Forward(ctx, ref.server, method, route, query, headers, body)
	if err != nil {
		return nil, err
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		response.Body.Close()
		switch response.StatusCode {
		case 404:
			return nil, ErrPathNotFound
		case 409:
			return nil, ErrPathExists
		case 422:
			return nil, ErrChecksumMismatch
		}
		return nil, ErrRemoteUnavailable
	}
	return response, nil
}
func decodeTransferResponse(response *http.Response, target interface{}) error {
	defer response.Body.Close()
	// Listings are paginated and bounded; never accept unbounded upstream JSON.
	if err := json.NewDecoder(io.LimitReader(response.Body, 4<<20)).Decode(target); err != nil {
		return ErrRemoteUnavailable
	}
	return nil
}
func (s *DesktopTransfers) destinationReady(ctx context.Context, dest transferPath) error {
	if dest.server == "" {
		_, _, err := s.files.ResolveForWrite(dest.path)
		return err
	}
	response, err := s.remote(ctx, dest, "GET", "/system/upload", nil, nil, nil)
	if err != nil {
		return err
	}
	var capability struct {
		CreateOnly bool `json:"createOnly"`
	}
	if err := decodeTransferResponse(response, &capability); err != nil {
		return err
	}
	if !capability.CreateOnly {
		return ErrInvalidOperation
	}
	return nil
}
func (s *DesktopTransfers) info(ctx context.Context, source transferPath) (*model.FileInfo, error) {
	if source.server == "" {
		return s.files.GetInfo(ctx, source.path)
	}
	response, err := s.remote(ctx, source, "GET", "/files/"+source.path, nil, nil, nil)
	if err != nil {
		return nil, err
	}
	var result struct {
		Name  string            `json:"name"`
		Size  int64             `json:"size"`
		IsDir *bool             `json:"isDir"`
		Items *[]model.FileInfo `json:"items"`
	}
	if err := decodeTransferResponse(response, &result); err != nil {
		return nil, err
	}
	if result.Items != nil {
		return &model.FileInfo{IsDir: true}, nil
	}
	if result.IsDir == nil || result.Size < 0 {
		return nil, ErrRemoteUnavailable
	}
	return &model.FileInfo{IsDir: *result.IsDir, Size: result.Size}, nil
}
func validTransferChild(name string) bool {
	ref, err := parseTransferPath(name)
	return err == nil && ref.server == "" && !strings.Contains(name, "/") && name != "@remote"
}
func (s *DesktopTransfers) copyTree(ctx context.Context, source, dest transferPath, depth int, remaining *int) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	if depth > 64 || *remaining <= 0 {
		return ErrDirectoryTooLarge
	}
	*remaining--
	info, err := s.info(ctx, source)
	if err != nil {
		return err
	}
	if !info.IsDir {
		return s.copyFile(ctx, source, dest, info.Size)
	}
	if dest.server == "" {
		err = CreateUploadDirectory(s.files, dest.path)
	} else {
		response, requestErr := s.remote(ctx, dest, "POST", "/stream/upload/"+dest.path, nil, http.Header{"X-Create-Only": {"true"}, "X-Upload-Type": {"directory"}}, nil)
		err = requestErr
		if response != nil {
			response.Body.Close()
		}
	}
	if err != nil {
		return err
	}
	for page := 1; ; page++ {
		var list model.FileList
		if source.server == "" {
			result, listErr := s.files.List(ctx, source.path, model.ListOptions{Page: page, PageSize: 100, IncludeHidden: true, SortBy: "name", SortDir: "asc"})
			if listErr != nil {
				return listErr
			}
			list = *result
		} else {
			response, requestErr := s.remote(ctx, source, "GET", "/files/list/"+source.path, url.Values{"page": {strconv.Itoa(page)}, "pageSize": {"100"}, "includeHidden": {"true"}, "sortBy": {"name"}, "sortDir": {"asc"}}, nil, nil)
			if requestErr != nil {
				return requestErr
			}
			if err := decodeTransferResponse(response, &list); err != nil {
				return err
			}
		}
		if len(list.Items) > 100 || list.TotalCount < 0 || list.TotalCount > 10000 {
			return ErrDirectoryTooLarge
		}
		for _, child := range list.Items {
			if !validTransferChild(child.Name) {
				return ErrRemoteUnavailable
			}
			nextSource := source
			nextSource.path += "/" + child.Name
			nextDest := dest
			nextDest.path += "/" + child.Name
			if err := s.copyTree(ctx, nextSource, nextDest, depth+1, remaining); err != nil {
				return err
			}
		}
		if page*100 >= list.TotalCount {
			return nil
		}
		if len(list.Items) == 0 {
			return ErrRemoteUnavailable
		}
	}
}
func (s *DesktopTransfers) open(ctx context.Context, source transferPath) (io.ReadCloser, error) {
	if source.server == "" {
		file, _, err := s.files.OpenFile(ctx, source.path)
		return file, err
	}
	response, err := s.remote(ctx, source, "GET", "/stream/download/"+source.path, nil, nil, nil)
	if err != nil {
		return nil, err
	}
	return response.Body, nil
}
func (s *DesktopTransfers) copyFile(ctx context.Context, source, dest transferPath, size int64) error {
	input, err := s.open(ctx, source)
	if err != nil {
		return err
	}
	defer input.Close()
	// Close local reads as well as HTTP reads when a job is cancelled.
	done := make(chan struct{})
	defer close(done)
	go func() {
		select {
		case <-ctx.Done():
			input.Close()
		case <-done:
		}
	}()
	total := int64(0)
	if size > 0 {
		total = (size-1)/transferChunkSize + 1
	}
	if total == 0 {
		total = 1
	}
	if total > int64(^uint(0)>>1) {
		return ErrInvalidOperation
	}
	id := uuid.New().String()
	if dest.server == "" {
		defer s.uploads.Discard(id)
	}
	hash := sha256.New()
	buffer := make([]byte, transferChunkSize)
	for index := int64(0); index < total; index++ {
		if err := ctx.Err(); err != nil {
			return err
		}
		count := min(int64(transferChunkSize), size-index*transferChunkSize)
		chunk := buffer[:int(count)]
		if _, err := io.ReadFull(input, chunk); err != nil {
			return err
		}
		hash.Write(chunk)
		checksum := ""
		if index == total-1 {
			var extra [1]byte
			n, err := input.Read(extra[:])
			if n != 0 || !errors.Is(err, io.EOF) {
				return ErrChecksumMismatch
			}
			checksum = "sha256:" + hex.EncodeToString(hash.Sum(nil))
		}
		metadata := UploadChunk{UploadID: id, ChunkIndex: int(index), TotalChunks: int(total), ChunkSize: transferChunkSize, TotalSize: size, Checksum: checksum, CreateOnly: true}
		if dest.server == "" {
			if _, err := s.uploads.AcceptChunk(ctx, dest.path, metadata, bytes.NewReader(chunk)); err != nil {
				return err
			}
		} else {
			headers := http.Header{"Content-Type": {"application/octet-stream"}, "X-Create-Only": {"true"}, "X-Chunk-Index": {fmt.Sprint(index)}, "X-Total-Chunks": {fmt.Sprint(total)}, "X-Chunk-Size": {fmt.Sprint(transferChunkSize)}, "X-Total-Size": {fmt.Sprint(size)}}
			headers.Set("X-Upload-ID", id)
			if checksum != "" {
				headers.Set("X-Checksum", checksum)
			}
			response, err := s.remote(ctx, dest, "POST", "/stream/upload/"+dest.path, nil, headers, bytes.NewReader(chunk))
			if err != nil {
				return err
			}
			var result UploadResult
			if err := decodeTransferResponse(response, &result); err != nil {
				return err
			}
			if result.Complete != (index == total-1) || result.UploadID != id {
				return ErrRemoteUnavailable
			}
		}
	}
	return nil
}
