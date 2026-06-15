import type { FileInfo } from '$lib/api/files';
import { getPreviewType } from '$lib/utils/fileTypes';

export type BackgroundImageMode = 'cover' | 'contain' | 'stretch' | 'center' | 'tile';

export const DEFAULT_BACKGROUND_IMAGE_MODE: BackgroundImageMode = 'cover';

export const WALLPAPER_DISPLAY_OPTIONS: Array<{ value: BackgroundImageMode; label: string }> = [
	{ value: 'cover', label: 'Crop to fill' },
	{ value: 'contain', label: 'Fit on screen' },
	{ value: 'stretch', label: 'Stretch' },
	{ value: 'center', label: 'Center' },
	{ value: 'tile', label: 'Tile' }
];

export function normalizeBackgroundImageMode(value: unknown): BackgroundImageMode {
	switch (value) {
		case 'cover':
		case 'contain':
		case 'stretch':
		case 'center':
		case 'tile':
			return value;
		default:
			return DEFAULT_BACKGROUND_IMAGE_MODE;
	}
}

export function getWallpaperBackgroundStyle(mode: BackgroundImageMode): {
	size: string;
	repeat: string;
	position: string;
} {
	switch (normalizeBackgroundImageMode(mode)) {
		case 'contain':
			return { size: 'contain', repeat: 'no-repeat', position: 'center' };
		case 'stretch':
			return { size: '100% 100%', repeat: 'no-repeat', position: 'center' };
		case 'center':
			return { size: 'auto', repeat: 'no-repeat', position: 'center' };
		case 'tile':
			return { size: 'auto', repeat: 'repeat', position: 'center' };
		case 'cover':
		default:
			return { size: 'cover', repeat: 'no-repeat', position: 'center' };
	}
}

export function isWallpaperImageFile(item: FileInfo): boolean {
	return (
		!item.isDir && (item.mimeType?.startsWith('image/') || getPreviewType(item.name) === 'image')
	);
}

export function readFileAsDataUrl(
	file: File,
	onProgress?: (progress: number) => void
): Promise<string> {
	return new Promise((resolve, reject) => {
		const reader = new FileReader();
		reader.onloadstart = () => onProgress?.(0);
		reader.onprogress = (event) => {
			if (event.lengthComputable && event.total > 0) {
				onProgress?.(Math.round((event.loaded / event.total) * 100));
			}
		};
		reader.onload = () => {
			if (typeof reader.result === 'string') {
				onProgress?.(100);
				resolve(reader.result);
			} else {
				reject(new Error('Unsupported file result'));
			}
		};
		reader.onerror = () => reject(reader.error ?? new Error('Unable to read file'));
		reader.readAsDataURL(file);
	});
}
