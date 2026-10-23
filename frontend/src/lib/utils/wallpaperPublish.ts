/** Publishes explicit wallpaper saves without re-uploading matching image bytes. */
import {
	deleteWallpaper,
	getWallpaper,
	updateWallpaperDisplay,
	uploadWallpaper
} from '#lib/api/wallpaper.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { resolveBackgroundImageUrl } from '#lib/utils/appearance.js';
import { normalizeBackgroundImageMode } from '#lib/utils/wallpaper.js';
import { sha256Hex } from '#lib/utils/wallpaperHash.js';
import {
	dataUrlToBlob,
	getLocalWallpaperBlob,
	isInlineWallpaperDataUrl,
	isLocalWallpaperReference,
	toLocalWallpaperReference
} from '#lib/utils/wallpaperStorage.js';

export async function publishWallpaper(signal: AbortSignal): Promise<void> {
	const { backgroundImage } = settingsStore.current;
	if (backgroundImage) {
		await reconcile(backgroundImage, signal);
	} else if (await getWallpaper(signal)) {
		signal.throwIfAborted();
		await deleteWallpaper(signal);
	}
	signal.throwIfAborted();
}

async function reconcile(backgroundImage: string, signal: AbortSignal) {
	const display = {
		mode: normalizeBackgroundImageMode(settingsStore.current.backgroundImageMode),
		frostedGlass: settingsStore.current.frostedGlass
	};
	const [stored, image] = await Promise.all([
		getWallpaper(signal),
		readWallpaperImage(backgroundImage, signal)
	]);
	signal.throwIfAborted();
	if (!image) throw new Error('The wallpaper image could not be read for sharing.');

	// Compare bytes when the browser can hash (secure origins): a server image
	// replaced at the same path keeps its source string.
	const hash = await sha256Hex(image);
	signal.throwIfAborted();
	const sameImage =
		stored !== null &&
		(hash !== null
			? hash === stored.sha256
			: stored.source === backgroundImage ||
				backgroundImage === toLocalWallpaperReference(stored.sha256));
	if (!sameImage) {
		await uploadWallpaper(image, display, backgroundImage, signal);
	} else if (stored.mode !== display.mode || stored.frostedGlass !== display.frostedGlass) {
		await updateWallpaperDisplay(display, signal);
	}
}

// Device wallpapers are read straight from browser storage: the page's CSP does
// not allow fetching blob: URLs. Server wallpapers come from the preview API.
async function readWallpaperImage(
	backgroundImage: string,
	signal: AbortSignal
): Promise<Blob | null> {
	if (isLocalWallpaperReference(backgroundImage)) return getLocalWallpaperBlob(backgroundImage);
	if (isInlineWallpaperDataUrl(backgroundImage)) return dataUrlToBlob(backgroundImage);

	const url = await resolveBackgroundImageUrl(backgroundImage);
	signal.throwIfAborted();
	const response = url ? await fetch(url, { signal }) : null;
	return response?.ok ? response.blob() : null;
}
