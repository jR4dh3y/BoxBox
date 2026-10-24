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
