/**
 * Validation and resolution of accent and wallpaper preferences.
 */

import { getPreviewUrl } from '#lib/api/files.js';
import {
	isInlineWallpaperDataUrl,
	isLocalWallpaperReference,
	resolveLocalWallpaperUrl
} from '#lib/utils/wallpaperStorage.js';

export const DEFAULT_ACCENT_COLOR = '#4a9eff';
export type AccentColorSource = 'wallpaper' | 'manual';

export function normalizeAccentColorSource(
	value: unknown,
	customColor: string | null
): AccentColorSource {
	if (value === 'wallpaper' || value === 'manual') return value;
	return customColor ? 'manual' : 'wallpaper';
}

const SERVER_BACKGROUND_PREFIX = 'boxbox-server:';

const HEX_COLOR_PATTERN = /^#[0-9a-f]{6}$/i;

export function normalizeAccentColor(color: string | null): string | null {
	if (!color) return null;
	const trimmed = color.trim();
	return HEX_COLOR_PATTERN.test(trimmed) ? trimmed.toLowerCase() : null;
}

export function isValidAccentColor(color: string | null): boolean {
	return color === null || normalizeAccentColor(color) !== null;
}

function isSupportedBackgroundImage(value: string): boolean {
	if (
		value.startsWith(SERVER_BACKGROUND_PREFIX) &&
		value.length > SERVER_BACKGROUND_PREFIX.length
	) {
		return true;
	}
	if (value.startsWith('/') && !value.startsWith('//')) return true;
	if (isInlineWallpaperDataUrl(value)) return true;
	if (isLocalWallpaperReference(value)) return true;

	try {
		const url = new URL(value);
		return url.protocol === 'http:' || url.protocol === 'https:';
	} catch {
		return false;
	}
}

export function normalizeBackgroundImage(backgroundImage: string | null): string | null {
	if (!backgroundImage) return null;
	const trimmed = backgroundImage.trim();
	return isSupportedBackgroundImage(trimmed) ? trimmed : null;
}

export function isValidBackgroundImage(backgroundImage: string | null): boolean {
	return backgroundImage === null || normalizeBackgroundImage(backgroundImage) !== null;
}

export function toServerBackgroundImage(path: string): string | null {
	const normalizedPath = path.trim().replace(/^\/+|\/+$/g, '');
	return normalizedPath ? `${SERVER_BACKGROUND_PREFIX}${normalizedPath}` : null;
}

function getServerBackgroundPath(backgroundImage: string): string | null {
	if (!backgroundImage.startsWith(SERVER_BACKGROUND_PREFIX)) return null;
	const path = backgroundImage
		.slice(SERVER_BACKGROUND_PREFIX.length)
		.trim()
		.replace(/^\/+|\/+$/g, '');
	return path || null;
}

/** The file name a wallpaper value points at, or null for a browser-stored image that has no name. */
export function getBackgroundImageLabel(backgroundImage: string | null): string | null {
	const normalized = normalizeBackgroundImage(backgroundImage);
	if (!normalized) return null;

	if (isInlineWallpaperDataUrl(normalized) || isLocalWallpaperReference(normalized)) {
		return null;
	}

	const serverPath = getServerBackgroundPath(normalized);
	const displayPath = serverPath ?? normalized;

	try {
		const url = new URL(displayPath);
		const fileName = url.pathname.split('/').filter(Boolean).pop();
		return fileName ? decodeURIComponent(fileName) : url.hostname;
	} catch {
		const fileName = displayPath.split('/').filter(Boolean).pop();
		if (!fileName) return 'Wallpaper selected';

		try {
			return decodeURIComponent(fileName);
		} catch {
			return fileName;
		}
	}
}

export function resolveBackgroundImage(backgroundImage: string | null): string | null {
	const normalized = normalizeBackgroundImage(backgroundImage);
	if (!normalized) return null;

	const serverPath = getServerBackgroundPath(normalized);
	if (isLocalWallpaperReference(normalized)) return null;
	return serverPath ? getPreviewUrl(serverPath) : normalized;
}

export async function resolveBackgroundImageUrl(
	backgroundImage: string | null
): Promise<string | null> {
	const normalized = normalizeBackgroundImage(backgroundImage);
	if (!normalized) return null;

	if (isLocalWallpaperReference(normalized)) {
		return resolveLocalWallpaperUrl(normalized);
	}

	return resolveBackgroundImage(normalized);
}
