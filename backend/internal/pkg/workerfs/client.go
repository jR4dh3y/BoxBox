//go:build linux

package workerfs

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/spf13/afero"

	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/fdconn"
	"github.com/jR4dh3y/BoxBox/backend/internal/pkg/filesystem"
)

// Dialer returns a fresh connection to a worker for this client's identity.
type Dialer func() (*fdconn.Conn, error)

// Client implements filesystem.FS by forwarding every call to a worker. It
// dials lazily, redials after the worker exits, and closes an idle connection
// so the worker can exit. Files it returns stay usable after that.
type Client struct {
	dial Dialer
	idle time.Duration

	mu      sync.Mutex
	current *session
}

var _ filesystem.FS = (*Client)(nil)

func NewClient(dial Dialer, idle time.Duration) *Client {
	return &Client{dial: dial, idle: idle}
}

type session struct {
	conn *fdconn.Conn

	mu       sync.Mutex
	nextID   uint64
	pending  map[uint64]chan fdconn.Message
	inFlight int
	idle     *time.Timer
	dead     bool
}

// Close disconnects from the worker. Later calls dial a new one.
func (c *Client) Close() error {
	c.mu.Lock()
	current := c.current
	c.current = nil
	c.mu.Unlock()
	if current != nil {
		return current.conn.Close()
	}
	return nil
}

func (c *Client) session() (*session, error) {
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.current != nil {
		c.current.mu.Lock()
		dead := c.current.dead
		c.current.mu.Unlock()
		if !dead {
			return c.current, nil
		}
	}
	conn, err := c.dial()
	if err != nil {
		return nil, fmt.Errorf("%w: %w", filesystem.ErrWorkerUnavailable, err)
	}
	current := &session{conn: conn, pending: make(map[uint64]chan fdconn.Message)}
	c.current = current
	go current.read()
	return current, nil
}

func (s *session) read() {
	for {
		message, err := s.conn.Receive()
		if err != nil {
			s.fail()
			return
		}
		s.mu.Lock()
		reply, ok := s.pending[message.ID]
		delete(s.pending, message.ID)
		s.mu.Unlock()
		if !ok {
			if message.File != nil {
				_ = message.File.Close()
			}
			continue
		}
		reply <- message
	}
}

// fail ends every call still waiting; their callers see filesystem.ErrWorkerUnavailable.
func (s *session) fail() {
	s.mu.Lock()
	s.dead = true
	pending := s.pending
	s.pending = map[uint64]chan fdconn.Message{}
	if s.idle != nil {
		s.idle.Stop()
	}
	s.mu.Unlock()
	_ = s.conn.Close()
	for _, reply := range pending {
		close(reply)
	}
}

func (c *Client) call(req request) (response, fdconn.Message, error) {
	payload, err := json.Marshal(req)
	if err != nil {
		return response{}, fdconn.Message{}, err
	}
	current, err := c.session()
	if err != nil {
		return response{}, fdconn.Message{}, err
	}
	reply := make(chan fdconn.Message, 1)
	current.mu.Lock()
	if current.dead {
		current.mu.Unlock()
		return response{}, fdconn.Message{}, filesystem.ErrWorkerUnavailable
	}
	current.nextID++
	id := current.nextID
	current.pending[id] = reply
	current.inFlight++
	if current.idle != nil {
		current.idle.Stop()
	}
	current.mu.Unlock()
	defer c.release(current)

	if err := current.conn.Send(id, payload, nil); err != nil {
		current.fail()
		return response{}, fdconn.Message{}, fmt.Errorf("%w: %w", filesystem.ErrWorkerUnavailable, err)
	}
	message, ok := <-reply
	if !ok {
		return response{}, fdconn.Message{}, filesystem.ErrWorkerUnavailable
	}
	var decoded response
	if err := json.Unmarshal(message.Payload, &decoded); err != nil {
		if message.File != nil {
			_ = message.File.Close()
		}
		return response{}, fdconn.Message{}, fmt.Errorf("workerfs: bad response: %w", err)
	}
	if decoded.Err != nil {
		if message.File != nil {
			_ = message.File.Close()
		}
		return response{}, fdconn.Message{}, decoded.Err.decode()
	}
	return decoded, message, nil
}

func (c *Client) release(current *session) {
	current.mu.Lock()
	defer current.mu.Unlock()
	current.inFlight--
	if current.inFlight > 0 || current.dead || c.idle <= 0 {
		return
	}
	current.idle = time.AfterFunc(c.idle, func() {
		current.mu.Lock()
		idle := current.inFlight == 0 && !current.dead
		current.mu.Unlock()
		if !idle {
			return
		}
		c.mu.Lock()
		if c.current == current {
			c.current = nil
		}
		c.mu.Unlock()
		// The reader sees the close and marks the session dead.
		_ = current.conn.Close()
	})
}

func (c *Client) do(req request) error {
	_, _, err := c.call(req)
	return err
}

func (c *Client) ReadDir(name string) ([]fs.DirEntry, error) {
	reply, _, err := c.call(request{Op: opReadDir, Path: name})
	if err != nil {
		return nil, err
	}
	return entries(reply.Infos), nil
}

func (c *Client) ReadDirLimit(name string, limit int) ([]fs.DirEntry, bool, error) {
	reply, _, err := c.call(request{Op: opReadDirLimit, Path: name, Limit: limit})
	if err != nil {
		return nil, false, err
	}
	return entries(reply.Infos), reply.Truncated, nil
}

func (c *Client) ReadDirFileInfoLimit(name string, limit int) ([]fs.FileInfo, bool, error) {
	reply, _, err := c.call(request{Op: opReadDirFileInfoLimit, Path: name, Limit: limit})
	if err != nil {
		return nil, false, err
	}
	if reply.Truncated {
		return nil, true, nil
	}
	infos := make([]fs.FileInfo, len(reply.Infos))
	for i, wire := range reply.Infos {
		infos[i] = fileInfo{wire}
	}
	return infos, false, nil
}

func (c *Client) EvalSymlinks(path string) (string, error) {
	reply, _, err := c.call(request{Op: opEvalSymlinks, Path: path})
	return reply.Text, err
}

func (c *Client) Readlink(path string) (string, error) {
	reply, _, err := c.call(request{Op: opReadlink, Path: path})
	return reply.Text, err
}

func (c *Client) Stat(name string) (fs.FileInfo, error) {
	return c.info(request{Op: opStat, Path: name})
}

func (c *Client) Lstat(name string) (fs.FileInfo, error) {
	return c.info(request{Op: opLstat, Path: name})
}

func (c *Client) info(req request) (fs.FileInfo, error) {
	reply, _, err := c.call(req)
	if err != nil {
		return nil, err
	}
	if reply.Info == nil {
		return nil, errors.New("workerfs: response without file info")
	}
	return fileInfo{*reply.Info}, nil
}

func (c *Client) Open(name string) (afero.File, error) {
	return c.OpenFile(name, os.O_RDONLY, 0)
}

func (c *Client) Create(name string) (afero.File, error) {
	return c.OpenFile(name, os.O_RDWR|os.O_CREATE|os.O_TRUNC, 0o666)
}

// OpenFile returns the descriptor the worker opened, so reads and writes run
// in this process at full speed after the worker's identity was checked.
func (c *Client) OpenFile(name string, flag int, perm os.FileMode) (afero.File, error) {
	_, message, err := c.call(request{Op: opOpenFile, Path: name, Flag: flag, Perm: perm})
	if err != nil {
		return nil, err
	}
	file := message.OpenFile(filepath.Clean("/" + name))
	if file == nil {
		return nil, errors.New("workerfs: worker did not send the opened file")
	}
	return file, nil
}

func (c *Client) Remove(name string) error {
	return c.do(request{Op: opRemove, Path: name})
}

func (c *Client) RemoveAll(path string) error {
	return c.do(request{Op: opRemoveAll, Path: path})
}

func (c *Client) Rename(oldpath, newpath string) error {
	return c.do(request{Op: opRename, Path: oldpath, Path2: newpath})
}

func (c *Client) RenameNoReplace(oldpath, newpath string) error {
	return c.do(request{Op: opRenameNoReplace, Path: oldpath, Path2: newpath})
}

func (c *Client) MkdirAllWithin(root, path string, perm os.FileMode) error {
	return c.do(request{Op: opMkdirAllWithin, Path: root, Path2: path, Perm: perm})
}

func (c *Client) MkdirAll(path string, perm os.FileMode) error {
	return c.do(request{Op: opMkdirAll, Path: path, Perm: perm})
}

func (c *Client) Mkdir(path string, perm os.FileMode) error {
	return c.do(request{Op: opMkdir, Path: path, Perm: perm})
}

func (c *Client) Chmod(name string, mode os.FileMode) error {
	return c.do(request{Op: opChmod, Path: name, Perm: mode})
}

func (c *Client) Exists(path string) (bool, error) {
	reply, _, err := c.call(request{Op: opExists, Path: path})
	return reply.OK, err
}

func (c *Client) IsDir(path string) (bool, error) {
	reply, _, err := c.call(request{Op: opIsDir, Path: path})
	return reply.OK, err
}

func (c *Client) WriteFile(name string, data []byte, perm os.FileMode) error {
	return c.do(request{Op: opWriteFile, Path: name, Data: data, Perm: perm})
}

func (c *Client) ReadFile(name string) ([]byte, error) {
	reply, _, err := c.call(request{Op: opReadFile, Path: name})
	return reply.Data, err
}

func entries(infos []wireInfo) []fs.DirEntry {
	result := make([]fs.DirEntry, len(infos))
	for i, wire := range infos {
		result[i] = dirEntry{fileInfo{wire}}
	}
	return result
}
