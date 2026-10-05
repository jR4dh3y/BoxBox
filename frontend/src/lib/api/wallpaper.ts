/**
 * Wallpaper API client - stores the user's wallpaper on the server so their share pages can show it.
 */

import { ApiRequestError, apiRequest } from './client';
import type { BackgroundImageMode } from '$lib/utils/wallpaper';

export interface WallpaperDisplay {
	mode: BackgroundImageMode;
	frostedGlass: boolean;
}

export interface StoredWallpaper extends WallpaperDisplay {
	/** The wallpaper setting value the image was uploaded from. */
	source: string;
}

/** The wallpaper stored on the server, or null when there is none. */
export async function getWallpaper(): Promise<StoredWallpaper | null> {
	try {
		return await apiRequest<StoredWallpaper>('/settings/wallpaper');
	} catch (error) {
		if (error instanceof ApiRequestError && error.status === 404) return null;
		throw error;
	}
}

/** Upload a new wallpaper image with how it should be drawn and where it came from. */
export async function uploadWallpaper(
	image: Blob,
	display: WallpaperDisplay,
	source: string
): Promise<void> {
	await apiRequest('/settings/wallpaper', {
		method: 'PUT',
		body: image,
		params: { mode: display.mode, frostedGlass: String(display.frostedGlass), source }
	});
}

/** Change how the stored wallpaper is drawn without re-uploading it. */
export async function updateWallpaperDisplay(display: WallpaperDisplay): Promise<void> {
	await apiRequest('/settings/wallpaper', { method: 'PATCH', body: display });
}

/** Remove the stored wallpaper. */
export async function deleteWallpaper(): Promise<void> {
	await apiRequest('/settings/wallpaper', { method: 'DELETE' });
}
