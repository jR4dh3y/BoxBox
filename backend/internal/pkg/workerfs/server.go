//go:build linux

package workerfs

import (
	"encoding/json"
	"errors"
	"fmt"
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
			if errors.Is(err, net.ErrClosed) {
				return nil
			}
			return err
		}
		if message.File != nil {
			_ = message.File.Close()
		}
		wg.Go(func() {
			reply, file := handle(fsys, message.Payload)
			payload, err := json.Marshal(reply)
			if err != nil {
				payload, _ = json.Marshal(response{Err: encodeError(err)})
			}
			// A failed send means the API is gone; Receive reports it next.
			_ = conn.Send(message.ID, payload, file)
			if file != nil {
				_ = file.Close()
			}
		})
	}
}

func handle(fsys filesystem.FS, payload []byte) (response, *os.File) {
	var req request
	if err := json.Unmarshal(payload, &req); err != nil {
		return response{Err: encodeError(fmt.Errorf("workerfs: bad request: %w", err))}, nil
	}
	fail := func(err error) response { return response{Err: encodeError(err)} }
	switch req.Op {
	case opReadDir:
		entries, err := fsys.ReadDir(req.Path)
		if err != nil {
			return fail(err), nil
		}
		return response{Infos: entryInfos(entries)}, nil
	case opReadDirLimit:
		entries, truncated, err := fsys.ReadDirLimit(req.Path, req.Limit)
		if err != nil {
			return fail(err), nil
		}
		return response{Infos: entryInfos(entries), Truncated: truncated}, nil
	case opReadDirFileInfoLimit:
		infos, truncated, err := fsys.ReadDirFileInfoLimit(req.Path, req.Limit)
		if err != nil {
			return fail(err), nil
		}
		wire := make([]wireInfo, len(infos))
		for i, info := range infos {
			wire[i] = toWireInfo(info)
		}
		return response{Infos: wire, Truncated: truncated}, nil
	case opEvalSymlinks:
		text, err := fsys.EvalSymlinks(req.Path)
		return textReply(text, err), nil
	case opReadlink:
		text, err := fsys.Readlink(req.Path)
		return textReply(text, err), nil
	case opStat:
		return infoReply(fsys.Stat(req.Path)), nil
	case opLstat:
		return infoReply(fsys.Lstat(req.Path)), nil
	case opOpenFile:
		opened, err := fsys.OpenFile(req.Path, req.Flag, req.Perm)
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
		return errReply(fsys.Remove(req.Path)), nil
	case opRemoveAll:
		return errReply(fsys.RemoveAll(req.Path)), nil
	case opRename:
		return errReply(fsys.Rename(req.Path, req.Path2)), nil
	case opRenameNoReplace:
		return errReply(fsys.RenameNoReplace(req.Path, req.Path2)), nil
	case opMkdirAllWithin:
		return errReply(fsys.MkdirAllWithin(req.Path, req.Path2, req.Perm)), nil
	case opMkdirAll:
		return errReply(fsys.MkdirAll(req.Path, req.Perm)), nil
	case opMkdir:
		return errReply(fsys.Mkdir(req.Path, req.Perm)), nil
	case opChmod:
		return errReply(fsys.Chmod(req.Path, req.Perm)), nil
	case opExists:
		ok, err := fsys.Exists(req.Path)
		return boolReply(ok, err), nil
	case opIsDir:
		ok, err := fsys.IsDir(req.Path)
		return boolReply(ok, err), nil
	case opWriteFile:
		return errReply(fsys.WriteFile(req.Path, req.Data, req.Perm)), nil
	case opReadFile:
		data, err := fsys.ReadFile(req.Path)
		if err != nil {
			return fail(err), nil
		}
		return response{Data: data}, nil
	}
	return fail(fmt.Errorf("workerfs: unknown operation %q", req.Op)), nil
}

// Directory entries carry their metadata so callers never need a round trip
// per entry. An entry that vanished before Info is left out, like os.ReadDir.
func entryInfos(entries []fs.DirEntry) []wireInfo {
	wire := make([]wireInfo, 0, len(entries))
	for _, entry := range entries {
		info, err := entry.Info()
		if err != nil {
			continue
		}
		wire = append(wire, toWireInfo(info))
	}
	return wire
}

func errReply(err error) response { return response{Err: encodeError(err)} }

func textReply(text string, err error) response {
	return response{Text: text, Err: encodeError(err)}
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
