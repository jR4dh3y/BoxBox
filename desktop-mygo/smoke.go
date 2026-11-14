package main

import "os"

// paintProbeScript is Electrobun's smoke probe: with BOXBOX_DESKTOP_SMOKE=1,
// the page requests /api/v1/_desktop-painted/<path> two frames after a
// folder's first row is laid out, so the server log times the journey the
// user sees for both desktop hosts alike.
func paintProbeScript() string {
	if os.Getenv("BOXBOX_DESKTOP_SMOKE") != "1" {
		return ""
	}
	return `;(() => {
	const seen = new Set();
	const report = () => {
		const path = new URLSearchParams(location.search).get('path');
		const row = [...document.querySelectorAll('[data-file-path]')].find((row) => row.dataset.filePath?.startsWith(path + '/'));
		if (!path || !row || seen.has(path) || row.getBoundingClientRect().height === 0) return;
		seen.add(path);
		requestAnimationFrame(() => requestAnimationFrame(() => { fetch('/api/v1/_desktop-painted/' + path); }));
	};
	const start = () => {
		new MutationObserver(report).observe(document, { subtree: true, childList: true, attributes: true });
		report();
	};
	if (document.documentElement) start();
	else document.addEventListener('DOMContentLoaded', start, { once: true });
})();`
}
