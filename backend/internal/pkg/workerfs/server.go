//go:build linux

package workerfs

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"net"
	"os"
	"sync"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fdconn"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

// Serve answers requests on conn with fsys until the API closes the
// connection. Requests run concurrently; each reply carries the request ID.
func Serve(conn *fdconn.Conn, fsys filesystem.FS) error {
	var wg sync.WaitGroup
	defer wg.Wait()
	for {
		message, err := conn.Receive()
		if err != nil {
			// The API closing the socket is the normal way a worker ends.
			if errors.Is(err, net.ErrClosed) || errors.Is(err, io.EOF) {
				return nil
			}
			return err
		}
		wg.Go(func() {
			reply, file := handle(fsys, message.Payload, message.File)
			defer func() {
				if file != nil {
					_ = file.Close()
				}
			}()
			payload, err := json.Marshal(reply)
			if err == nil && len(payload) > fdconn.MaxMessageSize {
				err = fdconn.ErrMessageTooLarge
			}
			if err != nil {
				payload, _ = json.Marshal(response{Err: encodeError(err)})
				_ = file.Close()
				file = nil
			}
			// Closing makes every waiting call fail instead of hang; the API
			// redials. Usually the API is already gone.
			if err := conn.Send(message.ID, payload, file); err != nil {
				_ = conn.Close()
			}
		})
	}
}

// handle owns sent, the file the API attached to the request, if any.
func handle(fsys filesystem.FS, payload []byte, sent *os.File) (response, *os.File) {
	if sent != nil {
		defer sent.Close()
	}
	var req request
	if err := json.Unmarshal(payload, &req); err != nil {
		return response{Err: encodeError(fmt.Errorf("workerfs: bad request: %w", err))}, nil
	}
	path, path2 := string(req.Path), string(req.Path2)
	fail := func(err error) response { return response{Err: encodeError(err)} }
	switch req.Op {
	case opReadDir:
		entries, err := fsys.ReadDir(path)
		if err != nil {
			return fail(err), nil
		}
		return response{Infos: entryInfos(entries)}, nil
	case opReadDirLimit:
		entries, truncated, err := fsys.ReadDirLimit(path, req.Limit)
		if err != nil {
			return fail(err), nil
		}
		return response{Infos: entryInfos(entries), Truncated: truncated}, nil
	case opReadDirFileInfoLimit:
		infos, truncated, err := fsys.ReadDirFileInfoLimit(path, req.Limit)
		if err != nil {
			return fail(err), nil
		}
		wire := make([]wireInfo, len(infos))
		for i, info := range infos {
			wire[i] = toWireInfo(info)
		}
		return response{Infos: wire, Truncated: truncated}, nil
	case opEvalSymlinks:
		text, err := fsys.EvalSymlinks(path)
		return textReply(text, err), nil
	case opReadlink:
		text, err := fsys.Readlink(path)
		return textReply(text, err), nil
	case opSymlink:
		return errReply(fsys.Symlink(path, path2)), nil
	case opStat:
		return infoReply(fsys.Stat(path)), nil
	case opLstat:
		return infoReply(fsys.Lstat(path)), nil
	case opOpenFile:
		opened, err := fsys.OpenFile(path, req.Flag, req.Perm)
		if err != nil {
			return fail(err), nil
		}
		file, ok := opened.(*os.File)
		if !ok {
			_ = opened.Close()
			return fail(errors.New("workerfs: worker filesystem returned a non-OS file")), nil
		}
		return response{}, file
	case opRemove:
		return errReply(fsys.Remove(path)), nil
	case opRemoveEmptyDirectory:
		return errReply(fsys.RemoveEmptyDirectory(path)), nil
	case opRemoveAll:
		return errReply(fsys.RemoveAll(path)), nil
	case opRename:
		return errReply(fsys.Rename(path, path2)), nil
	case opRenameNoReplace:
		return errReply(fsys.RenameNoReplace(path, path2)), nil
	case opMkdirAllWithin:
		return errReply(fsys.MkdirAllWithin(path, path2, req.Perm)), nil
	case opMkdirAll:
		return errReply(fsys.MkdirAll(path, req.Perm)), nil
	case opMkdir:
		return errReply(fsys.Mkdir(path, req.Perm)), nil
	case opChmod:
		return errReply(fsys.Chmod(path, req.Perm)), nil
	case opChmodFile:
		if sent == nil {
			return fail(errors.New("workerfs: chmodFile without a file")), nil
		}
		return errReply(fsys.ChmodFile(sent, req.Perm)), nil
	case opExists:
		ok, err := fsys.Exists(path)
		return boolReply(ok, err), nil
	case opIsDir:
		ok, err := fsys.IsDir(path)
		return boolReply(ok, err), nil
	case opWriteFile:
		return errReply(fsys.WriteFile(path, req.Data, req.Perm)), nil
	case opReadFile:
		data, err := fsys.ReadFile(path)
		if err != nil {
			return fail(err), nil
		}
		return response{Data: data}, nil
	}
	return fail(fmt.Errorf("workerfs: unknown operation %q", req.Op)), nil
}

// Directory entries carry their metadata so callers never need a round trip
// per entry. Every Info error goes to the caller, as with a local ReadDir: a
// move must abort on an entry that vanished, not delete it uncopied.
func entryInfos(entries []fs.DirEntry) []wireInfo {
	wire := make([]wireInfo, len(entries))
	for i, entry := range entries {
		info, err := entry.Info()
		if err != nil {
			wire[i] = wireInfo{Name: rawString(entry.Name()), Mode: entry.Type(), Err: encodeError(err)}
			continue
		}
		wire[i] = toWireInfo(info)
	}
	return wire
}

func errReply(err error) response { return response{Err: encodeError(err)} }

func textReply(text string, err error) response {
	return response{Text: rawString(text), Err: encodeError(err)}
}

func boolReply(ok bool, err error) response {
	return response{OK: ok, Err: encodeError(err)}
}

func infoReply(info fs.FileInfo, err error) response {
	if err != nil {
		return response{Err: encodeError(err)}
	}
	wire := toWireInfo(info)
	return response{Info: &wire}
}
