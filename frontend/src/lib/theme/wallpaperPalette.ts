import {
	argbFromRgb,
	Hct,
	hexFromArgb,
	labFromArgb,
	QuantizerMap,
	QuantizerWu,
	Score,
	xyzFromArgb
} from '@material/material-color-utilities';
import type { WallpaperPalette } from './wallpaperPaletteProtocol.js';

/** Quantize actual colors before ranking their coverage and suitability for an accent. */
export function wallpaperPaletteFromPixels(pixels: Uint8ClampedArray): WallpaperPalette | null {
	const opaque: number[] = [];
	for (let index = 0; index + 3 < pixels.length; index += 4) {
		if (pixels[index + 3] < 128) continue;
		opaque.push(argbFromRgb(pixels[index], pixels[index + 1], pixels[index + 2]));
	}
	if (opaque.length === 0) return null;

	// Fixed centers keep an unchanged wallpaper's palette stable across reloads.
	const centers = new QuantizerWu().quantize(opaque, 64).map((color) => ({
		color,
		lab: labFromArgb(color)
	}));
	if (centers.length === 0) return null;
	const candidates = new Map<number, number>();
	for (const [color, count] of QuantizerMap.quantize(opaque)) {
		const point = labFromArgb(color);
		let nearest = centers[0].color;
		let shortest = Infinity;
		for (const center of centers) {
			const distance =
				(point[0] - center.lab[0]) ** 2 +
				(point[1] - center.lab[1]) ** 2 +
				(point[2] - center.lab[2]) ** 2;
			if (distance < shortest) {
				shortest = distance;
				nearest = center.color;
			}
		}
		candidates.set(nearest, (candidates.get(nearest) ?? 0) + count);
	}
	for (const color of candidates.keys()) {
		const { chroma, tone } = Hct.fromInt(color);
		if (chroma < 12 || tone < 12 || tone > 98) candidates.delete(color);
	}
	if (candidates.size === 0) return null;
	const seed = Score.score(candidates, { desired: 1, fallbackColorARGB: 0 })[0];
	if (!seed) return null;
	const source = Hct.fromInt(seed);
	const chroma = Math.max(48, Math.min(72, source.chroma));
	const color = hexFromArgb(Hct.from(source.hue, chroma, 65).toInt());
	const iconColor = hexFromArgb(Hct.from(source.hue, chroma, 78).toInt());

	// Text gets its own tone; contrast must not wash the control color toward white.
	const background = xyzFromArgb(argbFromRgb(104, 104, 104))[1] / 100;
	for (let tone = 76; tone <= 100; tone++) {
		const text = Hct.from(source.hue, chroma, tone).toInt();
		if ((xyzFromArgb(text)[1] / 100 + 0.05) / (background + 0.05) >= 4.5)
			return { color, textColor: hexFromArgb(text), iconColor };
	}
	return { color, textColor: '#ffffff', iconColor };
}
