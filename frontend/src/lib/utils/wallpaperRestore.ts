/** Downloads and caches server image bytes only when a wallpaper exists. */
import type { UserSettings } from '#lib/stores/settings.svelte.js';
import { retryPendingWallpaper } from './wallpaperPublish.js';
import { downloadWallpaper } from '#lib/api/wallpaperImage.js';
import type { StoredWallpaper } from '#lib/api/wallpaper.js';
import { getBackgroundImageLabel } from '#lib/utils/appearance.js';
import { sha256Hex } from '#lib/utils/wallpaperHash.js';
import {
	saveLocalWallpaperBlob,
	getLocalWallpaperBlob,
	toLocalWallpaperReference
} from '#lib/utils/wallpaperStorage.js';

export type RestoredWallpaper = Pick<
	UserSettings,
	'backgroundImage' | 'backgroundImageName' | 'backgroundImageMode' | 'frostedGlass'
>;

export async function restoreWallpaper(
	stored: StoredWallpaper | null,
	initial: UserSettings,
	signal: AbortSignal,
	username: string
): Promise<RestoredWallpaper> {
	if (await retryPendingWallpaper(signal))
		return {
			backgroundImage: initial.backgroundImage,
			backgroundImageName: initial.backgroundImageName,
			backgroundImageMode: initial.backgroundImageMode,
			frostedGlass: initial.frostedGlass
		};
	if (!stored)
		return {
			backgroundImage: null,
			backgroundImageName: null,
			backgroundImageMode: 'cover',
			frostedGlass: false
		};
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
	// A server file names itself; an upload keeps the name only in the browser it came from.
	const backgroundImageName =
		getBackgroundImageLabel(stored.source) ??
		(stored.source === initial.backgroundImage || initial.backgroundImage === reference
			? initial.backgroundImageName
			: null);
	return {
		backgroundImage: reference,
		backgroundImageName,
		backgroundImageMode: stored.mode,
		frostedGlass: stored.frostedGlass
	};
}
