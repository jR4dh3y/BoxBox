/**
 * Makes the wallpaper stored on the server, which share pages show, match the
 * wallpaper this browser shows. Runs after login and whenever settings are saved.
 */
import {
	deleteWallpaper,
	getWallpaper,
	updateWallpaperDisplay,
	uploadWallpaper,
	type WallpaperDisplay
} from '$lib/api/wallpaper';
import { resolveBackgroundImageUrl } from '$lib/stores/settings';
import {
	dataUrlToBlob,
	getLocalWallpaperBlob,
	isInlineWallpaperDataUrl,
	isLocalWallpaperReference
} from '$lib/utils/wallpaperStorage';

export async function syncShareWallpaper(
	backgroundImage: string | null,
	display: WallpaperDisplay
): Promise<void> {
	const stored = await getWallpaper();

	if (!backgroundImage) {
		if (stored) await deleteWallpaper();
		return;
	}

	if (stored?.source === backgroundImage) {
		if (stored.mode !== display.mode || stored.frostedGlass !== display.frostedGlass) {
			await updateWallpaperDisplay(display);
		}
		return;
	}

	const image = await readWallpaperImage(backgroundImage);
	if (!image) throw new Error('The wallpaper image could not be read for sharing.');
	await uploadWallpaper(image, display, backgroundImage);
}

// Device wallpapers are read straight from browser storage: the page's CSP
// does not allow fetching blob: URLs. Server wallpapers come from the preview API.
async function readWallpaperImage(backgroundImage: string): Promise<Blob | null> {
	if (isLocalWallpaperReference(backgroundImage)) return getLocalWallpaperBlob(backgroundImage);
	if (isInlineWallpaperDataUrl(backgroundImage)) return dataUrlToBlob(backgroundImage);

	const url = await resolveBackgroundImageUrl(backgroundImage);
	const response = url ? await fetch(url) : null;
	return response?.ok ? response.blob() : null;
}
