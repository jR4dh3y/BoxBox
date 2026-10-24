/**
 * Wallpaper API client - stores the user's wallpaper on the server so their share pages can show it.
 */

import { ApiRequestError, apiRequest } from './client';
import type { BackgroundImageMode } from '#lib/utils/wallpaper.js';
import { normalizeBackgroundImageMode } from '#lib/utils/wallpaper.js';

export interface WallpaperDisplay {
	mode: BackgroundImageMode;
	frostedGlass: boolean;
}

export interface StoredWallpaper extends WallpaperDisplay {
	/** The wallpaper setting value the image was uploaded from. */
	source: string;
	/** Hex SHA-256 of the stored image bytes. */
	sha256: string;
}

/** The wallpaper stored on the server, or null when there is none. */
export async function getWallpaper(signal?: AbortSignal): Promise<StoredWallpaper | null> {
	try {
		const value: unknown = await apiRequest('/settings/wallpaper', { signal });
		if (
			typeof value !== 'object' ||
			value === null ||
			!('mode' in value) ||
			value.mode !== normalizeBackgroundImageMode(value.mode) ||
			!('frostedGlass' in value) ||
			typeof value.frostedGlass !== 'boolean' ||
			!('source' in value) ||
			typeof value.source !== 'string' ||
			!('sha256' in value) ||
			typeof value.sha256 !== 'string' ||
			!/^[a-f0-9]{64}$/.test(value.sha256)
		)
			throw new Error('Invalid wallpaper metadata');
		return {
			mode: normalizeBackgroundImageMode(value.mode),
			frostedGlass: value.frostedGlass,
			source: value.source,
			sha256: value.sha256
		};
	} catch (error) {
		if (error instanceof ApiRequestError && error.status === 404) return null;
		throw error;
	}
}
