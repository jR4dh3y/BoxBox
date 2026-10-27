import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { normalizeAccentColorSource } from '#lib/utils/appearance.js';
import { luminance, parseHexColor, solidAccentColor } from './colors.js';
import { readableWallpaperAccent, wallpaperAccentFromPixels } from './wallpaperAccent.js';

function pixels(...groups: Array<{ color: [number, number, number, number]; count: number }>) {
	return new Uint8ClampedArray(
		groups.flatMap(({ color, count }) => Array.from({ length: count }, () => color).flat())
	);
}

describe('wallpaper accent', () => {
	it('follows the dominant colored region rather than a small vivid detail', () => {
		const color = wallpaperAccentFromPixels(
			pixels(
				{ color: [100, 140, 210, 255], count: 100 },
				{ color: [255, 0, 0, 255], count: 5 },
				{ color: [245, 245, 245, 255], count: 300 }
			)
		);
		assert.ok(color);
		const [red, green, blue] = parseHexColor(color);
		assert.ok(blue > green && green > red);
	});

	it('ignores transparent pixels and does not invent a hue for a monochrome image', () => {
		assert.equal(
			wallpaperAccentFromPixels(
				pixels(
					{ color: [255, 0, 0, 0], count: 100 },
					{ color: [255, 255, 255, 255], count: 100 },
					{ color: [0, 0, 0, 255], count: 100 },
					{ color: [120, 120, 120, 255], count: 100 }
				)
			),
			null
		);
		assert.equal(wallpaperAccentFromPixels(new Uint8ClampedArray()), null);
	});

	it('keeps accent text and filled buttons readable for dark, saturated and pale palettes', () => {
		for (const sample of ['#001133', '#d41428', '#00ff11', '#1d4ed8', '#f4e2b7', '#ffffff']) {
			const accent = readableWallpaperAccent(sample);
			assert.ok(
				(luminance(parseHexColor(accent)) + 0.05) / (luminance([85, 85, 85]) + 0.05) >= 4.5
			);
			assert.ok(1.05 / (luminance(parseHexColor(solidAccentColor(accent))) + 0.05) >= 4.5);
		}
	});

	it('preserves existing custom colors while enabling automatic colors for the default theme', () => {
		assert.equal(normalizeAccentColorSource(undefined, '#ab1234'), 'manual');
		assert.equal(normalizeAccentColorSource(undefined, null), 'wallpaper');
		assert.equal(normalizeAccentColorSource('wallpaper', '#ab1234'), 'wallpaper');
		assert.equal(normalizeAccentColorSource('manual', null), 'manual');
		assert.equal(normalizeAccentColorSource({ unexpected: true }, null), 'wallpaper');
	});
});
