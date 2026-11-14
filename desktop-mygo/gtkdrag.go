package main

import (
	"fmt"
	"net/url"
	"path/filepath"
	"slices"
	"strings"
	"sync"
	"unsafe"

	"github.com/ebitengine/purego"
)

// WebKitGTK hides the paths of files other apps drop from the page, and
// sends the uri-list a page sets for a drag out as empty bytes. gtkDrag
// watches the GTK drag signals of the WebKitWebView to fill both gaps.
// GTK callbacks run on the main thread; Outgoing is set from the page's
// binding goroutine, hence the mutex.
type gtkDrag struct {
	mu       sync.Mutex
	outgoing string // file URLs the page is dragging out, one per line
	dragging bool   // a drag started in a BoxBox page is under way
}

var gtk struct {
	once sync.Once
	err  error

	containerGetChildren func(uintptr) uintptr
	containerGetType     func() uintptr
	typeCheckInstanceIsA func(uintptr, uintptr) bool
	typeNameFromInstance func(uintptr) string
	listFree             func(uintptr)
	signalConnectData    func(instance uintptr, signal string, handler, data, destroy uintptr, flags uint32) uint64
	selectionGetTarget   func(uintptr) uintptr
	selectionGetData     func(uintptr) uintptr
	selectionGetLength   func(uintptr) int32
	selectionSetURIs     func(sel, uris uintptr) bool
	strSplit             func(text, delimiter string, max int32) uintptr
	strFreev             func(uintptr)
	atomInternStatic     func(string) uintptr

	uriList, portalFiles, portalFileTransfer uintptr
}

func loadGTK() error {
	gtk.once.Do(func() {
		lib, err := purego.Dlopen("libgtk-3.so.0", purego.RTLD_NOW|purego.RTLD_GLOBAL)
		if err != nil {
			gtk.err = err
			return
		}
		purego.RegisterLibFunc(&gtk.containerGetChildren, lib, "gtk_container_get_children")
		purego.RegisterLibFunc(&gtk.containerGetType, lib, "gtk_container_get_type")
		purego.RegisterLibFunc(&gtk.typeCheckInstanceIsA, lib, "g_type_check_instance_is_a")
		purego.RegisterLibFunc(&gtk.typeNameFromInstance, lib, "g_type_name_from_instance")
		purego.RegisterLibFunc(&gtk.listFree, lib, "g_list_free")
		purego.RegisterLibFunc(&gtk.signalConnectData, lib, "g_signal_connect_data")
		purego.RegisterLibFunc(&gtk.selectionGetTarget, lib, "gtk_selection_data_get_target")
		purego.RegisterLibFunc(&gtk.selectionGetData, lib, "gtk_selection_data_get_data")
		purego.RegisterLibFunc(&gtk.selectionGetLength, lib, "gtk_selection_data_get_length")
		purego.RegisterLibFunc(&gtk.selectionSetURIs, lib, "gtk_selection_data_set_uris")
		purego.RegisterLibFunc(&gtk.strSplit, lib, "g_strsplit")
		purego.RegisterLibFunc(&gtk.strFreev, lib, "g_strfreev")
		purego.RegisterLibFunc(&gtk.atomInternStatic, lib, "gdk_atom_intern_static_string")
		gtk.uriList = gtk.atomInternStatic("text/uri-list")
		// GTK 3.24.37+ hands files to sandboxed and GTK4 apps through the FileTransfer portal.
		gtk.portalFiles = gtk.atomInternStatic("application/vnd.portal.files")
		gtk.portalFileTransfer = gtk.atomInternStatic("application/vnd.portal.filetransfer")
	})
	return gtk.err
}

// findWebView returns the WebKitWebView inside a GTK widget tree.
func findWebView(widget uintptr) uintptr {
	if gtk.typeNameFromInstance(widget) == "WebKitWebView" {
		return widget
	}
	if !gtk.typeCheckInstanceIsA(widget, gtk.containerGetType()) {
		return 0
	}
	children := gtk.containerGetChildren(widget)
	defer gtk.listFree(children)
	// GList: data, next, prev.
	for node := children; node != 0; node = *(*uintptr)(unsafe.Add(ptr(node), unsafe.Sizeof(uintptr(0)))) {
		if found := findWebView(*(*uintptr)(ptr(node))); found != 0 {
			return found
		}
	}
	return 0
}

// ptr converts an address GTK owns; Go never moves or frees that memory.
func ptr(address uintptr) unsafe.Pointer {
	return *(*unsafe.Pointer)(unsafe.Pointer(&address))
}

const connectAfter = 1 // G_CONNECT_AFTER

// attach connects to the WebKitWebView in window, a GtkWindow*. onIncoming
// is told the local paths of files another app drags over it.
// The callbacks live as long as the process, as purego callbacks must.
func (d *gtkDrag) attach(window uintptr, onIncoming func(paths []string)) error {
	if err := loadGTK(); err != nil {
		return err
	}
	web := findWebView(window)
	if web == 0 {
		return fmt.Errorf("no WebKitWebView in the window")
	}
	gtk.signalConnectData(web, "drag-begin", purego.NewCallback(func(widget, context, data uintptr) {
		d.mu.Lock()
		d.dragging = true
		d.mu.Unlock()
	}), 0, 0, 0)
	gtk.signalConnectData(web, "drag-end", purego.NewCallback(func(widget, context, data uintptr) {
		d.mu.Lock()
		d.dragging, d.outgoing = false, ""
		d.mu.Unlock()
	}), 0, 0, 0)
	// After WebKit's handler, which writes an empty file list.
	gtk.signalConnectData(web, "drag-data-get", purego.NewCallback(func(widget, context, sel uintptr, info, time uint32, data uintptr) {
		d.mu.Lock()
		list := d.outgoing
		d.mu.Unlock()
		target := gtk.selectionGetTarget(sel)
		if list == "" || (target != gtk.uriList && target != gtk.portalFiles && target != gtk.portalFileTransfer) {
			return
		}
		// GTK writes the uri-list, or registers the files with the portal.
		uris := gtk.strSplit(list, "\n", -1)
		gtk.selectionSetURIs(sel, uris)
		gtk.strFreev(uris)
	}), 0, 0, connectAfter)
	var incoming []string
	// WebKit asks for a drag's data while it moves over the page.
	gtk.signalConnectData(web, "drag-data-received", purego.NewCallback(func(widget, context uintptr, x, y int32, sel uintptr, info, time uint32, data uintptr) {
		if gtk.selectionGetTarget(sel) != gtk.uriList {
			return
		}
		d.mu.Lock()
		own := d.dragging
		d.mu.Unlock()
		if own {
			return
		}
		if paths := parseURIList(selectionBytes(sel)); !slices.Equal(paths, incoming) {
			incoming = paths
			onIncoming(paths)
		}
	}), 0, 0, 0)
	return nil
}

func selectionBytes(sel uintptr) []byte {
	length := gtk.selectionGetLength(sel)
	data := gtk.selectionGetData(sel)
	if length <= 0 || data == 0 {
		return nil
	}
	return append([]byte(nil), unsafe.Slice((*byte)(ptr(data)), length)...)
}

// parseURIList returns the local paths of the file URLs in a text/uri-list.
func parseURIList(list []byte) []string {
	var paths []string
	for _, line := range strings.Split(string(list), "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		u, err := url.Parse(line)
		if err != nil || u.Scheme != "file" || (u.Host != "" && u.Host != "localhost") || !filepath.IsAbs(u.Path) {
			continue
		}
		paths = append(paths, filepath.Clean(u.Path))
	}
	return paths
}

func fileURL(path string) string { return (&url.URL{Scheme: "file", Path: path}).String() }
