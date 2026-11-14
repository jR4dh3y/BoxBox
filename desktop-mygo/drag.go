package main

import (
	"encoding/json"
	"fmt"
	"log"
	"strings"

	"github.com/egoist/mygo"
)

// DesktopDrag is bound for the page: it names the files a drag out carries.
type DesktopDrag struct {
	server *desktopServer
	drag   *gtkDrag
}

// Start takes the virtual paths of the items being dragged out of the page.
// Remote items have no local file and are left out.
func (s *DesktopDrag) Start(paths []string) {
	locals := make([]string, 0, len(paths))
	for _, path := range paths {
		if local, ok := s.server.localPath(path); ok {
			locals = append(locals, local)
		}
	}
	urls := make([]string, len(locals))
	for i, local := range locals {
		urls[i] = fileURL(local)
	}
	s.drag.mu.Lock()
	s.drag.outgoing = strings.Join(urls, "\n")
	s.drag.mu.Unlock()
}

// attachDrag hooks the window's WebKitWebView. Dropped files reach the page
// as an ordinary drop carrying their file URLs, which the shared frontend
// already reads, so it needs no knowledge of this host.
func attachDrag(win *mygo.Window, drag *gtkDrag) {
	onIncoming := func(paths []string) {
		urls := make([]string, len(paths))
		for i, path := range paths {
			urls[i] = fileURL(path)
		}
		uris, _ := json.Marshal(urls)
		script := fmt.Sprintf("window.__boxboxHost && window.__boxboxHost.setIncoming(%s); return null;", uris)
		// Eval waits for the page, which needs the main loop this runs on.
		go func() {
			if _, err := win.Page().Eval(script); err != nil {
				log.Printf("Could not pass dragged files to the page: %v", err)
			}
		}()
	}
	if err := drag.attach(win.NativeHandle(), onIncoming); err != nil {
		log.Printf("Drag and drop with other apps is unavailable: %v", err)
	}
}

// dragScript runs in the page. WebKitGTK loses file data both ways, so:
//   - A drag out sets a uri-list, so WebKit offers one, and tells the host
//     which files to put in it.
//   - Once a uri-list is offered, WebKit hands the page's own drops that
//     instead of BoxBox's item data; dragover and drop are sent again with it.
//   - A drop from another app arrives without its files; it is sent again
//     with the file URLs the host reported.
func dragScript(origin string) string {
	originJSON, _ := json.Marshal(origin)
	return fmt.Sprintf(`(() => {
	if (location.origin !== %s) return;
	const FILES = 'application/x-boxbox-files';
	let own = null;
	let incoming = [];
	Object.defineProperty(window, '__boxboxHost', {
		value: Object.freeze({ setIncoming(uris) { incoming = Array.isArray(uris) ? uris.filter((uri) => typeof uri === 'string') : []; } })
	});
	window.addEventListener('dragstart', (event) => {
		const transfer = event.dataTransfer;
		own = null;
		if (!transfer || !window.mygo) return;
		const files = transfer.getData(FILES);
		let items;
		try { items = JSON.parse(files); } catch { return; }
		if (!Array.isArray(items)) return;
		const paths = items.map((item) => item && item.path).filter((path) => typeof path === 'string');
		if (!paths.length) return;
		own = { files, text: transfer.getData('text/plain') };
		// WebKit offers a uri-list only when it holds a valid URL, then sends it
		// empty; the host writes the real file URLs.
		transfer.setData('text/uri-list', 'file:///');
		window.mygo.call('DesktopDrag.Start', paths).catch(() => {});
	});
	window.addEventListener('dragend', () => { own = null; }, true);
	const resend = (event, data) => {
		event.stopImmediatePropagation();
		const transfer = new DataTransfer();
		for (const [type, value] of data) transfer.setData(type, value);
		const copy = new DragEvent(event.type, {
			bubbles: true, cancelable: true, composed: true,
			clientX: event.clientX, clientY: event.clientY, screenX: event.screenX, screenY: event.screenY,
			ctrlKey: event.ctrlKey, shiftKey: event.shiftKey, altKey: event.altKey, metaKey: event.metaKey,
			dataTransfer: transfer
		});
		event.target.dispatchEvent(copy);
		// A constructed DataTransfer keeps no dropEffect; WebKit picks one from the modifiers.
		if (copy.defaultPrevented || event.type === 'drop') event.preventDefault();
	};
	const ownData = (event) => {
		const types = Array.from(event.dataTransfer ? event.dataTransfer.types : []);
		return own && event.isTrusted && !types.includes(FILES) ? [[FILES, own.files], ['text/plain', own.text]] : null;
	};
	window.addEventListener('dragover', (event) => {
		const data = ownData(event);
		if (data) resend(event, data);
	}, true);
	window.addEventListener('drop', (event) => {
		const data = ownData(event);
		if (data) {
			own = null;
			resend(event, data);
			return;
		}
		const transfer = event.dataTransfer;
		if (!event.isTrusted || !transfer || !incoming.length || transfer.files.length) return;
		const types = Array.from(transfer.types);
		if (!types.includes('text/uri-list') || types.includes(FILES) || transfer.getData('text/uri-list')) return;
		const uris = incoming;
		incoming = [];
		resend(event, [['text/uri-list', uris.join('\r\n')]]);
	}, true);
})();`, originJSON)
}
