/**
 * Restores the server wallpaper on login and publishes explicit settings saves.
 * One queue and session cancellation keep work from crossing account changes.
 */
import { wallpaperPublication } from './wallpaperPublication.js';
import { getWallpaper } from '#lib/api/wallpaper.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { currentUsername } from '#lib/utils/sessionUser.js';

import { beginShareWallpaperLoginSync, enqueueWallpaper } from './wallpaperSession';
export { resetShareWallpaperLoginSync } from './wallpaperSession';

/**
 * The server is authoritative on login, including wallpapers removed elsewhere.
 */
export function syncShareWallpaperOnLogin(): Promise<void> {
	const signal = beginShareWallpaperLoginSync();
	if (!signal) return Promise.resolve();
	const revision = settingsStore.wallpaperRevision;
	return enqueueWallpaper(async () => {
		signal.throwIfAborted();
		const user = currentUsername();
		if (!user) return;
		const initial = settingsStore.current;
		const stored = await getWallpaper(signal);
		signal.throwIfAborted();
		if (
			!stored &&
			!initial.backgroundImage &&
			initial.backgroundImageMode === 'cover' &&
			!initial.frostedGlass &&
			wallpaperPublication(user).kind !== 'pending'
		)
			return;
		const { restoreSessionWallpaper } = await import('./wallpaperRestore.js');
		signal.throwIfAborted();
		await restoreSessionWallpaper(stored, initial, signal, user, revision);
	}).catch((error) => {
		if (!signal.aborted) console.warn('Unable to restore the wallpaper', error);
	});
}
