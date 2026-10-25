/** Downloads and caches server image bytes only when a wallpaper exists. */
import { settingsStore, type UserSettings } from '#lib/stores/settings.svelte.js';
import { currentUsername } from './sessionUser.js';
import { retryPendingWallpaper } from './wallpaperPublish.js';
import { downloadWallpaper } from '#lib/api/wallpaperImage.js';
import type { StoredWallpaper } from '#lib/api/wallpaper.js';
import { sha256Hex } from '#lib/utils/wallpaperHash.js';
import {
	deleteLocalWallpaper,
	saveLocalWallpaperBlob,
	getLocalWallpaperBlob,
	toLocalWallpaperReference
} from '#lib/utils/wallpaperStorage.js';

async function restoreWallpaper(
	stored: StoredWallpaper | null,
	initial: UserSettings,
	signal: AbortSignal,
	username: string
): Promise<Pick<UserSettings, 'backgroundImage' | 'backgroundImageMode' | 'frostedGlass'>> {
	if (await retryPendingWallpaper(signal))
		return {
			backgroundImage: initial.backgroundImage,
			backgroundImageMode: initial.backgroundImageMode,
			frostedGlass: initial.frostedGlass
		};
	if (!stored) return { backgroundImage: null, backgroundImageMode: 'cover', frostedGlass: false };
	const id = `${stored.sha256}.${encodeURIComponent(username)}`;
	const reference = toLocalWallpaperReference(id);
	if (!(await getLocalWallpaperBlob(reference))) {
		const image =
			(await getLocalWallpaperBlob(toLocalWallpaperReference(stored.sha256))) ??
			(await downloadWallpaper(signal));
		signal.throwIfAborted();
		const hash = await sha256Hex(image);
		if (hash !== null && hash !== stored.sha256)
			throw new Error('Wallpaper changed during download');
		signal.throwIfAborted();
		await saveLocalWallpaperBlob(image, id);
	}
	return {
		backgroundImage: reference,
		backgroundImageMode: stored.mode,
		frostedGlass: stored.frostedGlass
	};
}

/** Apply a login result only while its account and wallpaper edit still own the session. */
export async function restoreSessionWallpaper(
	stored: StoredWallpaper | null,
	initial: UserSettings,
	signal: AbortSignal,
	username: string,
	revision: number
): Promise<void> {
	const wallpaper = await restoreWallpaper(stored, initial, signal, username);
	signal.throwIfAborted();
	if (currentUsername() !== username || settingsStore.wallpaperRevision !== revision) return;
	settingsStore.set({ ...settingsStore.current, ...wallpaper });
	if (initial.backgroundImage !== wallpaper.backgroundImage)
		await deleteLocalWallpaper(initial.backgroundImage);
}
