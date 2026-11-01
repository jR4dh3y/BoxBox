let controller = new AbortController();
let loginStarted = false;

export function beginShareWallpaperLoginSync(): AbortSignal | null {
	if (loginStarted) return null;
	loginStarted = true;
	return controller.signal;
}

export function shareWallpaperSignal(): AbortSignal {
	return controller.signal;
}

/** Logout must cancel queued work before another account can sign in. */
export function resetShareWallpaperLoginSync(): void {
	controller.abort();
	controller = new AbortController();
	loginStarted = false;
}

let queue: Promise<void> = Promise.resolve();

/** Login restoration and explicit publication share one ordered session queue. */
export function enqueueWallpaper(task: () => Promise<void>): Promise<void> {
	const run = queue.then(task);
	queue = run.catch(() => {});
	return run;
}
