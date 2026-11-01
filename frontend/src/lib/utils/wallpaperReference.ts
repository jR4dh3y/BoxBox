const LOCAL_WALLPAPER_PREFIX = 'boxbox-local-wallpaper:';
const LOCAL_WALLPAPER_ID_PATTERN = /^[a-z0-9_-]{8,}(?:\..+)?$/i;
const INLINE_WALLPAPER_PATTERN = /^data:image\/(avif|gif|jpeg|jpg|png|svg\+xml|webp);base64,/i;

export function isInlineWallpaperDataUrl(value: string): boolean {
	return INLINE_WALLPAPER_PATTERN.test(value.trim());
}

export function toLocalWallpaperReference(id: string): string {
	return `${LOCAL_WALLPAPER_PREFIX}${id}`;
}

export function getLocalWallpaperId(reference: string): string | null {
	const trimmed = reference.trim();
	if (!trimmed.startsWith(LOCAL_WALLPAPER_PREFIX)) return null;

	const id = trimmed.slice(LOCAL_WALLPAPER_PREFIX.length);
	return LOCAL_WALLPAPER_ID_PATTERN.test(id) ? id : null;
}

export function isLocalWallpaperReference(value: string): boolean {
	return getLocalWallpaperId(value) !== null;
}
