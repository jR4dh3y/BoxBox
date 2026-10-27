import { luminance, mixColor, parseHexColor, toHexColor } from './colors.js';

interface ColorGroup {
	weight: number;
	red: number;
	green: number;
	blue: number;
}

/** Keep accent text readable even over the brightest translucent wallpaper surface. */
export function readableWallpaperAccent(color: string): string {
	const background = luminance([85, 85, 85]);
	for (let step = 0; step <= 20; step++) {
		const candidate = mixColor(color, '#ffffff', step / 20);
		if ((luminance(parseHexColor(candidate)) + 0.05) / (background + 0.05) >= 4.5) return candidate;
	}
	return '#ffffff';
}

/** Group nearby hues so a small vivid detail cannot outweigh the wallpaper's main palette. */
export function wallpaperAccentFromPixels(pixels: Uint8ClampedArray): string | null {
	const groups = new Map<number, ColorGroup>();
	for (let index = 0; index + 3 < pixels.length; index += 4) {
		const red = pixels[index];
		const green = pixels[index + 1];
		const blue = pixels[index + 2];
		const alpha = pixels[index + 3] / 255;
		const high = Math.max(red, green, blue);
		const low = Math.min(red, green, blue);
		const range = high - low;
		const saturation = high === 0 ? 0 : range / high;
		if (alpha < 0.5 || high < 30 || low > 245 || saturation < 0.12) continue;

		const hue =
			60 *
			(high === red
				? (green - blue) / range
				: high === green
					? 2 + (blue - red) / range
					: 4 + (red - green) / range);
		const key = Math.floor(((hue + 360) % 360) / 15);
		const weight = alpha * (0.4 + saturation);
		const group = groups.get(key) ?? { weight: 0, red: 0, green: 0, blue: 0 };
		group.weight += weight;
		group.red += red * weight;
		group.green += green * weight;
		group.blue += blue * weight;
		groups.set(key, group);
	}

	let dominant: ColorGroup | null = null;
	for (const group of groups.values()) {
		if (!dominant || group.weight > dominant.weight) dominant = group;
	}
	return dominant
		? readableWallpaperAccent(
				toHexColor([
					dominant.red / dominant.weight,
					dominant.green / dominant.weight,
					dominant.blue / dominant.weight
				])
			)
		: null;
}

export async function sampleWallpaperAccent(
	url: string,
	signal: AbortSignal
): Promise<string | null> {
	signal.throwIfAborted();
	const image = new Image();
	image.crossOrigin = 'anonymous';
	image.decoding = 'async';
	const cancel = () => image.removeAttribute('src');
	signal.addEventListener('abort', cancel, { once: true });
	try {
		image.src = url;
		await image.decode();
		signal.throwIfAborted();
		// Sampling is bounded to 2,304 pixels and runs only when the wallpaper changes.
		const canvas = document.createElement('canvas');
		canvas.width = canvas.height = 48;
		const context = canvas.getContext('2d', { willReadFrequently: true });
		if (!context) return null;
		context.drawImage(image, 0, 0, 48, 48);
		return wallpaperAccentFromPixels(context.getImageData(0, 0, 48, 48).data);
	} finally {
		signal.removeEventListener('abort', cancel);
		cancel();
	}
}
