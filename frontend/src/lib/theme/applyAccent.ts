import { DEFAULT_ACCENT_COLOR } from '#lib/utils/appearance.js';
import { applyAccentColor } from './colors.js';
import type { AccentState } from './accent.svelte.js';
import type { WallpaperPalette } from './wallpaperPaletteProtocol.js';

export async function applyAccentTheme(
	customColor: string | null,
	wallpaper: string | null,
	signal: AbortSignal,
	onApplied: (state: AccentState, applied: boolean, wallpaperColor: string | null) => void,
	surfaceWallpaper: string | null = null
): Promise<void> {
	if (signal.aborted) return;
	const source = wallpaper ?? surfaceWallpaper;
	if (!source) {
		applyAccentColor(customColor);
		onApplied({ kind: 'manual' }, customColor !== null, null);
		return;
	}
	let palette: WallpaperPalette | null = null;
	try {
		const { sampleWallpaperAccent } = await import('./wallpaperAccent.js');
		signal.throwIfAborted();
		palette = await sampleWallpaperAccent(source, signal);
	} catch {
		signal.throwIfAborted();
	}
	if (signal.aborted) return;
	if (!wallpaper) {
		applyAccentColor(customColor);
		onApplied({ kind: 'manual' }, customColor !== null, palette?.color ?? null);
		return;
	}
	applyAccentColor(
		palette?.color ?? customColor ?? DEFAULT_ACCENT_COLOR,
		palette?.textColor ?? (customColor ? undefined : '#d0e0ff'),
		palette?.iconColor
	);
	onApplied(
		palette ? { kind: 'ready', color: palette.color } : { kind: 'unavailable' },
		true,
		palette?.color ?? null
	);
}
