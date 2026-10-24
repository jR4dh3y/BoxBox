/** Downloads and caches server image bytes only when a wallpaper exists. */
import type { UserSettings } from '#lib/stores/settings.svelte.js';
import { retryPendingWallpaper } from './wallpaperPublish.js';
import { downloadWallpaper } from '#lib/api/wallpaperImage.js';
import type { StoredWallpaper } from '#lib/api/wallpaper.js';
import { sha256Hex } from '#lib/utils/wallpaperHash.js';
import {
	saveLocalWallpaperBlob,
	getLocalWallpaperBlob,
	toLocalWallpaperReference
} from '#lib/utils/wallpaperStorage.js';

export async function restoreWallpaper(
	stored: StoredWallpaper | null,
	initial: UserSettings,
	signal: AbortSignal
): Promise<Pick<UserSettings, 'backgroundImage' | 'backgroundImageMode' | 'frostedGlass'>> {
	if (await retryPendingWallpaper(signal))
		return {
			backgroundImage: initial.backgroundImage,
			backgroundImageMode: initial.backgroundImageMode,
			frostedGlass: initial.frostedGlass
		};
	if (!stored) return { backgroundImage: null, backgroundImageMode: 'cover', frostedGlass: false };
	const reference = toLocalWallpaperReference(stored.sha256);
	if (!(await getLocalWallpaperBlob(reference))) {
		const image = await downloadWallpaper(signal);
		signal.throwIfAborted();
		const hash = await sha256Hex(image);
		if (hash !== null && hash !== stored.sha256)
			throw new Error('Wallpaper changed during download');
		signal.throwIfAborted();
		await saveLocalWallpaperBlob(image, stored.sha256);
	}
	return {
		backgroundImage: reference,
		backgroundImageMode: stored.mode,
		frostedGlass: stored.frostedGlass
	};
}
