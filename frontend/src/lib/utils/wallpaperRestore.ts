/** Restores the authenticated account's server wallpaper into its browser cache. */
import { downloadWallpaper, getWallpaper } from '#lib/api/wallpaper.js';
import { settingsStore, type UserSettings } from '#lib/stores/settings.svelte.js';
import { currentUsername } from '#lib/utils/sessionUser.js';
import { sha256Hex } from '#lib/utils/wallpaperHash.js';
import {
	deleteLocalWallpaper,
	getLocalWallpaperBlob,
	saveLocalWallpaperBlob,
	toLocalWallpaperReference
} from '#lib/utils/wallpaperStorage.js';

export async function restoreWallpaper(
	user: string,
	initial: UserSettings,
	signal: AbortSignal
): Promise<void> {
	const stored = await getWallpaper(signal);
	signal.throwIfAborted();
	let backgroundImage: string | null = null;
	if (stored) {
		backgroundImage = toLocalWallpaperReference(stored.sha256);
		if (!(await getLocalWallpaperBlob(backgroundImage))) {
			const image = await downloadWallpaper(signal);
			signal.throwIfAborted();
			const hash = await sha256Hex(image);
			if (hash !== null && hash !== stored.sha256)
				throw new Error('Wallpaper changed during download');
			signal.throwIfAborted();
			await saveLocalWallpaperBlob(image, stored.sha256);
		}
	}
	signal.throwIfAborted();
	// A save made while the request was pending wins over login restoration.
	if (
		currentUsername() !== user ||
		settingsStore.current.backgroundImage !== initial.backgroundImage ||
		settingsStore.current.backgroundImageMode !== initial.backgroundImageMode ||
		settingsStore.current.frostedGlass !== initial.frostedGlass
	)
		return;
	settingsStore.set({
		...settingsStore.current,
		backgroundImage,
		backgroundImageMode: stored?.mode ?? 'cover',
		frostedGlass: stored?.frostedGlass ?? false
	});
	if (initial.backgroundImage !== backgroundImage) {
		await deleteLocalWallpaper(initial.backgroundImage);
	}
}
