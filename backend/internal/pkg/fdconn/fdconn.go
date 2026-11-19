//go:build linux

// Package fdconn exchanges framed messages over a SOCK_SEQPACKET Unix socket.
// A message may carry one open file, delivered with its final frame. The NAS
// supervisor, API and file workers use it to hand each other sockets and files.
package fdconn

import (
	"encoding/binary"
	"errors"
	"fmt"
	"net"
	"os"
	"sync"
	"syscall"
)

const (
	headerSize = 9
	// Stays well under the default socket send buffer so a frame is never refused.
	maxFramePayload = 32 << 10
	// Bounds what a misbehaving peer can make the receiver buffer.
	MaxMessageSize = 64 << 20
	flagFinal      = 1
)

var ErrMessageTooLarge = errors.New("fdconn: message too large")

// Message is one complete message. File is nil unless the sender attached one;
// the receiver owns it and names it with OpenFile.
type Message struct {
	ID      uint64
	Payload []byte
	File    *os.File
}

// OpenFile renames a received file so errors and Name() report a real path.
func (m Message) OpenFile(name string) *os.File {
	if m.File == nil {
		return nil
	}
	fd, err := syscall.Dup(int(m.File.Fd()))
	_ = m.File.Close()
	if err != nil {
		return nil
	}
	syscall.CloseOnExec(fd)
	return os.NewFile(uintptr(fd), name)
}

// Conn is safe for concurrent Send calls. Receive must have a single caller.
type Conn struct {
	unix    *net.UnixConn
	writeMu sync.Mutex
	partial map[uint64][]byte
	pending int
	frame   []byte
	oob     []byte
}

// Pair returns a connected socket pair: one end as a Conn and the other as a
// file to hand to another process.
func Pair() (*Conn, *os.File, error) {
	fds, err := syscall.Socketpair(syscall.AF_UNIX, syscall.SOCK_SEQPACKET|syscall.SOCK_CLOEXEC, 0)
	if err != nil {
		return nil, nil, fmt.Errorf("fdconn: socketpair: %w", err)
	}
	local := os.NewFile(uintptr(fds[0]), "fdconn")
	peer := os.NewFile(uintptr(fds[1]), "fdconn-peer")
	conn, err := New(local)
	if err != nil {
		_ = peer.Close()
		return nil, nil, err
	}
	return conn, peer, nil
}

// New takes ownership of file, which must be a SOCK_SEQPACKET Unix socket.
func New(file *os.File) (*Conn, error) {
	defer file.Close()
	generic, err := net.FileConn(file)
	if err != nil {
		return nil, fmt.Errorf("fdconn: %w", err)
	}
	unix, ok := generic.(*net.UnixConn)
	if !ok {
		_ = generic.Close()
		return nil, errors.New("fdconn: not a Unix socket")
	}
	return &Conn{
		unix:    unix,
		partial: make(map[uint64][]byte),
		frame:   make([]byte, headerSize+maxFramePayload),
		oob:     make([]byte, syscall.CmsgSpace(4)),
	}, nil
}

// Send writes payload as one message. The caller keeps ownership of file.
func (c *Conn) Send(id uint64, payload []byte, file *os.File) error {
	if len(payload) > MaxMessageSize {
		return ErrMessageTooLarge
	}
	c.writeMu.Lock()
	defer c.writeMu.Unlock()
	frame := make([]byte, headerSize+min(len(payload), maxFramePayload))
	for {
		size := min(len(payload), maxFramePayload)
		final := size == len(payload)
		frame = frame[:headerSize+size]
		binary.LittleEndian.PutUint64(frame, id)
		frame[8] = 0
		var oob []byte
		if final {
			frame[8] = flagFinal
			if file != nil {
				oob = syscall.UnixRights(int(file.Fd()))
			}
		}
		copy(frame[headerSize:], payload[:size])
		if _, _, err := c.unix.WriteMsgUnix(frame, oob, nil); err != nil {
			return err
		}
		if final {
			return nil
		}
		payload = payload[size:]
	}
}

// Receive returns the next complete message.
func (c *Conn) Receive() (Message, error) {
	for {
		n, oobn, flags, _, err := c.unix.ReadMsgUnix(c.frame, c.oob)
		if err != nil {
			return Message{}, err
		}
		file, err := receivedFile(c.oob[:oobn])
		if err != nil {
			return Message{}, err
		}
		if n == 0 {
			// A zero-length datagram is how SOCK_SEQPACKET reports a closed peer.
			closeFile(file)
			return Message{}, net.ErrClosed
		}
		if flags&(syscall.MSG_TRUNC|syscall.MSG_CTRUNC) != 0 || n < headerSize {
			closeFile(file)
			return Message{}, errors.New("fdconn: malformed frame")
		}
		id := binary.LittleEndian.Uint64(c.frame)
		final := c.frame[8]&flagFinal != 0
		if !final && file != nil {
			closeFile(file)
			return Message{}, errors.New("fdconn: file attached before the final frame")
		}
		c.pending += n - headerSize
		if c.pending > MaxMessageSize {
			closeFile(file)
			return Message{}, ErrMessageTooLarge
		}
		payload := append(c.partial[id], c.frame[headerSize:n]...)
		if !final {
			c.partial[id] = payload
			continue
		}
		delete(c.partial, id)
		c.pending -= len(payload)
		return Message{ID: id, Payload: payload, File: file}, nil
	}
}

func (c *Conn) Close() error {
	return c.unix.Close()
}

func receivedFile(oob []byte) (*os.File, error) {
	if len(oob) == 0 {
		return nil, nil
	}
	messages, err := syscall.ParseSocketControlMessage(oob)
	if err != nil {
		return nil, fmt.Errorf("fdconn: control message: %w", err)
	}
	var file *os.File
	for _, message := range messages {
		fds, err := syscall.ParseUnixRights(&message)
		if err != nil {
			continue
		}
		for _, fd := range fds {
			if file == nil {
				file = os.NewFile(uintptr(fd), "received")
				continue
			}
			_ = syscall.Close(fd)
		}
	}
	return file, nil
}

func closeFile(file *os.File) {
	if file != nil {
		_ = file.Close()
	}
}
