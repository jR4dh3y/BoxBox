/**
 * Restores the server wallpaper on login and publishes explicit settings saves.
 * One queue and session cancellation keep work from crossing account changes.
 */
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { currentUsername } from '#lib/utils/sessionUser.js';

let queue: Promise<void> = Promise.resolve();
let loginSyncStarted = false;
let sessionController = new AbortController();

function enqueue(task: () => Promise<void>): Promise<void> {
	const run = queue.then(task);
	queue = run.catch(() => {});
	return run;
}

/**
 * The server is authoritative on login, including wallpapers removed elsewhere.
 */
export function syncShareWallpaperOnLogin(): Promise<void> {
	if (loginSyncStarted) return Promise.resolve();
	loginSyncStarted = true;
	const { signal } = sessionController;
	return enqueue(async () => {
		signal.throwIfAborted();
		const user = currentUsername();
		if (!user) return;
		const initial = settingsStore.current;
		const { restoreWallpaper } = await import('./wallpaperRestore.js');
		signal.throwIfAborted();
		await restoreWallpaper(user, initial, signal);
	}).catch((error) => {
		if (!signal.aborted) console.warn('Unable to restore the wallpaper', error);
	});
}

/** Allow the next authenticated session to sync, including after expiry. */
export function resetShareWallpaperLoginSync(): void {
	sessionController.abort();
	sessionController = new AbortController();
	loginSyncStarted = false;
}

/** After saving settings: make the server copy match exactly, including removal. */
export function publishShareWallpaper(): Promise<void> {
	const { signal } = sessionController;
	return enqueue(async () => {
		signal.throwIfAborted();
		const { publishWallpaper } = await import('./wallpaperPublish.js');
		signal.throwIfAborted();
		await publishWallpaper(signal);
	});
}
