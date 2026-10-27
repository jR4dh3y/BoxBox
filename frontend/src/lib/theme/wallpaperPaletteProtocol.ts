export interface WallpaperPalette {
	color: string;
	textColor: string;
	iconColor: string;
}

export function parseWallpaperPalette(value: unknown): WallpaperPalette | null {
	if (
		typeof value !== 'object' ||
		value === null ||
		!('color' in value) ||
		!('textColor' in value) ||
		!('iconColor' in value) ||
		typeof value.color !== 'string' ||
		typeof value.textColor !== 'string' ||
		typeof value.iconColor !== 'string' ||
		!/^#[0-9a-f]{6}$/i.test(value.color) ||
		!/^#[0-9a-f]{6}$/i.test(value.textColor) ||
		!/^#[0-9a-f]{6}$/i.test(value.iconColor)
	)
		return null;
	return { color: value.color, textColor: value.textColor, iconColor: value.iconColor };
}
