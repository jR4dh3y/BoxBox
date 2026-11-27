/** A disconnected watcher still revalidates visible data until SSE reconnects. */
export function watchDesktopDirectory(path: string, refresh: () => void): () => void {
	const source = new EventSource(`/api/v1/desktop/watch?${new URLSearchParams({ path })}`);
	let fallback: ReturnType<typeof setInterval> | undefined;
	const stopFallback = () => {
		if (fallback) clearInterval(fallback);
		fallback = undefined;
	};
	source.addEventListener('ready', () => {
		stopFallback();
		refresh();
	});
	source.addEventListener('change', refresh);
	source.onerror = () => {
		fallback ??= setInterval(refresh, 15_000);
	};
	const onFocus = () => refresh();
	window.addEventListener('focus', onFocus);
	return () => {
		stopFallback();
		source.close();
		window.removeEventListener('focus', onFocus);
	};
}
