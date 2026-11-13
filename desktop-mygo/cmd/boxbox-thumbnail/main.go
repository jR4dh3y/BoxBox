// Command boxbox-thumbnail renders one media thumbnail for the BoxBox server.
//
//	boxbox-thumbnail <libboxbox_thumbnail.so> <image|video|audio|pdf> <width> <height>
//
// The source file is open on descriptor 3; the PNG goes to stdout. Exit codes:
// 2 invalid request or source, 3 over a size limit, 4 renderer unavailable.
package main

import (
	"fmt"
	"os"
	"slices"
	"strconv"

	"github.com/ebitengine/purego"
)

var kinds = []string{"image", "video", "audio", "pdf"}

type failure struct {
	code    int
	message string
}

func main() {
	image, err := render(os.Args[1:])
	if err != nil {
		fmt.Fprintln(os.Stderr, err.message)
		os.Exit(err.code)
	}
	if _, writeErr := os.Stdout.Write(image); writeErr != nil {
		os.Exit(4)
	}
}

func render(args []string) ([]byte, *failure) {
	if len(args) != 4 || !slices.Contains(kinds, args[1]) {
		return nil, &failure{2, "Invalid thumbnail request"}
	}
	width, errW := strconv.Atoi(args[2])
	height, errH := strconv.Atoi(args[3])
	if errW != nil || errH != nil || width < 16 || width > 512 || height < 16 || height > 512 {
		return nil, &failure{2, "Invalid thumbnail dimensions"}
	}
	source := os.NewFile(3, "source")
	if info, err := source.Stat(); err != nil || !info.Mode().IsRegular() {
		return nil, &failure{2, "Thumbnail source must be a regular file"}
	}
	lib, err := purego.Dlopen(args[0], purego.RTLD_NOW|purego.RTLD_LOCAL)
	if err != nil {
		return nil, &failure{4, "Native thumbnail library unavailable"}
	}
	var abi func() uint32
	var fromFD func(fd, kind, width, height uint32, output *byte, capacity uint64) int32
	purego.RegisterLibFunc(&abi, lib, "boxbox_thumbnail_abi")
	purego.RegisterLibFunc(&fromFD, lib, "boxbox_thumbnail_fd")
	if abi() != 1 {
		return nil, &failure{4, "Unsupported thumbnail renderer ABI"}
	}
	output := make([]byte, 1<<20)
	n := fromFD(3, uint32(slices.Index(kinds, args[1])), uint32(width), uint32(height), &output[0], uint64(len(output)))
	switch {
	case n > 0 && int(n) <= len(output):
		return output[:n], nil
	case n == -2:
		return nil, &failure{3, "Thumbnail source could not be rendered"}
	case n == -4:
		return nil, &failure{4, "Thumbnail source could not be rendered"}
	default:
		return nil, &failure{2, "Thumbnail source could not be rendered"}
	}
}
