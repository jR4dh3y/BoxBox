import { formatFileSize } from '#lib/utils/format.js';

/** Matches the server's sniffed wallpaper types, so a picked file is never refused on save. */
const WALLPAPER_TYPES = new Set([
	'image/jpeg',
	'image/png',
	'image/gif',
	'image/webp',
	'image/avif',
	'image/svg+xml'
]);
const WALLPAPER_EXTENSIONS = new Set(['jpg', 'jpeg', 'png', 'gif', 'webp', 'avif', 'svg']);

export const WALLPAPER_MAX_BYTES = 20 * 1024 * 1024;

/** The picker's filter. Extensions cover systems that report no MIME type for a file. */
export const WALLPAPER_FILE_ACCEPT = [
	...WALLPAPER_TYPES,
	...[...WALLPAPER_EXTENSIONS].map((extension) => `.${extension}`)
].join(',');

/** Returns why the picked file cannot be a wallpaper, or null when it can. */
export function wallpaperFileProblem(file: Pick<File, 'name' | 'type' | 'size'>): string | null {
	const extension = file.name.split('.').pop()?.toLowerCase() ?? '';
	const supported = file.type
		? WALLPAPER_TYPES.has(file.type)
		: file.name.includes('.') && WALLPAPER_EXTENSIONS.has(extension);
	if (!supported) return 'Choose a JPEG, PNG, GIF, WebP, AVIF or SVG image.';
	if (file.size === 0) return 'This image is empty.';
	if (file.size > WALLPAPER_MAX_BYTES)
		return `Choose an image smaller than ${formatFileSize(WALLPAPER_MAX_BYTES)}.`;
	return null;
}

/** Rejects a file whose name and type say image but whose bytes do not decode. */
export async function decodeWallpaperImage(source: string): Promise<void> {
	const image = new Image();
	image.src = source;
	await image.decode();
}
