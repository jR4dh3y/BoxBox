/**
 * Restores the server wallpaper on login and publishes explicit settings saves.
 * One queue and session cancellation keep work from crossing account changes.
 */
import { wallpaperPublication } from './wallpaperPublication.js';
import { getWallpaper } from '#lib/api/wallpaper.js';
import { deleteLocalWallpaper } from '#lib/utils/wallpaperStorage.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { currentUsername } from '#lib/utils/sessionUser.js';

let queue: Promise<void> = Promise.resolve();

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
	const revision = settingsStore.wallpaperRevision;
	return enqueue(async () => {
		signal.throwIfAborted();
		const user = currentUsername();
		if (!user) return;
		const initial = settingsStore.current;
		const stored = await getWallpaper(signal);
		signal.throwIfAborted();
		let wallpaper: Pick<
			typeof initial,
			'backgroundImage' | 'backgroundImageName' | 'backgroundImageMode' | 'frostedGlass'
		> = {
			backgroundImage: null,
			backgroundImageName: null,
			backgroundImageMode: 'cover',
			frostedGlass: false
		};
		if (stored || initial.backgroundImage || wallpaperPublication(user).kind === 'pending') {
			const { restoreWallpaper } = await import('./wallpaperRestore.js');
			signal.throwIfAborted();
			wallpaper = await restoreWallpaper(stored, initial, signal, user);
		}
		signal.throwIfAborted();
		if (currentUsername() !== user || settingsStore.wallpaperRevision !== revision) return;
		settingsStore.set({
			...settingsStore.current,
			...wallpaper
		});
		if (initial.backgroundImage !== wallpaper.backgroundImage)
			await deleteLocalWallpaper(initial.backgroundImage);
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

/** Change image bytes only for an explicit edit or a pending publication. */
export function publishShareWallpaper(
	publication?: string,
	displayChanged = false,
	operation: AbortSignal = sessionController.signal
): Promise<void> {
	const signal = AbortSignal.any([sessionController.signal, operation]);
	return enqueue(async () => {
		signal.throwIfAborted();
		const { publishWallpaper } = await import('./wallpaperPublish.js');
		signal.throwIfAborted();
		await publishWallpaper(signal, publication, displayChanged);
	});
}
