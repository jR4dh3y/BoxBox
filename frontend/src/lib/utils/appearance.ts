/**
 * Accent color and wallpaper helpers: validation, normalisation and theme application
 */

import { getPreviewUrl } from '$lib/api/files';
import {
	isInlineWallpaperDataUrl,
	isLocalWallpaperReference,
	resolveLocalWallpaperUrl
} from '$lib/utils/wallpaperStorage';

export const DEFAULT_ACCENT_COLOR = '#4a9eff';

const SERVER_BACKGROUND_PREFIX = 'boxbox-server:';

const HEX_COLOR_PATTERN = /^#[0-9a-f]{6}$/i;
const THEME_COLOR_PROPERTIES = [
	'--color-accent',
	'--color-accent-hover',
	'--color-accent-muted',
	'--color-border-focus',
	'--color-folder',
	'--color-selection',
	'--color-selection-hover'
];

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

export function getBackgroundImageLabel(backgroundImage: string | null): string | null {
	const normalized = normalizeBackgroundImage(backgroundImage);
	if (!normalized) return null;

	if (isInlineWallpaperDataUrl(normalized) || isLocalWallpaperReference(normalized)) {
		return 'Local image selected';
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

function parseHexColor(color: string): [number, number, number] {
	const hex = color.slice(1);
	return [
		Number.parseInt(hex.slice(0, 2), 16),
		Number.parseInt(hex.slice(2, 4), 16),
		Number.parseInt(hex.slice(4, 6), 16)
	];
}

function toHexColor([red, green, blue]: [number, number, number]): string {
	return `#${[red, green, blue]
		.map((value) => Math.round(value).toString(16).padStart(2, '0'))
		.join('')}`;
}

function mixColor(color: string, target: string, amount: number): string {
	const sourceRgb = parseHexColor(color);
	const targetRgb = parseHexColor(target);

	return toHexColor([
		sourceRgb[0] + (targetRgb[0] - sourceRgb[0]) * amount,
		sourceRgb[1] + (targetRgb[1] - sourceRgb[1]) * amount,
		sourceRgb[2] + (targetRgb[2] - sourceRgb[2]) * amount
	]);
}

export function applyAccentColor(accentColor: string | null): void {
	if (typeof document === 'undefined') return;

	const color = normalizeAccentColor(accentColor);
	const rootStyle = document.documentElement.style;

	if (!color) {
		for (const property of THEME_COLOR_PROPERTIES) {
			rootStyle.removeProperty(property);
		}
		return;
	}

	rootStyle.setProperty('--color-accent', color);
	rootStyle.setProperty('--color-border-focus', color);
	rootStyle.setProperty('--color-folder', color);
	rootStyle.setProperty('--color-accent-hover', mixColor(color, '#000000', 0.32));
	rootStyle.setProperty('--color-accent-muted', mixColor(color, '#1e1e1e', 0.55));
	rootStyle.setProperty('--color-selection', mixColor(color, '#1e1e1e', 0.55));
	rootStyle.setProperty('--color-selection-hover', mixColor(color, '#000000', 0.32));
}
