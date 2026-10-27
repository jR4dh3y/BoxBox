import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { argbFromHex, Hct } from '@material/material-color-utilities';
import { normalizeAccentColorSource } from '#lib/utils/appearance.js';
import { luminance, parseHexColor, solidAccentColor } from './colors.js';
import { wallpaperPaletteFromPixels } from './wallpaperPalette.js';
import { parseWallpaperPalette } from './wallpaperPaletteProtocol.js';

function pixels(...groups: Array<{ color: [number, number, number, number]; count: number }>) {
	return new Uint8ClampedArray(
		groups.flatMap(({ color, count }) => Array.from({ length: count }, () => color).flat())
	);
}

describe('wallpaper accent', () => {
	it('follows the dominant colored region rather than a small vivid detail', () => {
		const palette = wallpaperPaletteFromPixels(
			pixels(
				{ color: [100, 140, 210, 255], count: 100 },
				{ color: [255, 0, 0, 255], count: 5 },
				{ color: [245, 245, 245, 255], count: 300 }
			)
		);
		assert.ok(palette);
		const hue = Hct.fromInt(argbFromHex(palette.color)).hue;
		assert.ok(hue > 230 && hue < 300);
	});

	it('ignores transparent pixels and does not invent a hue for a monochrome image', () => {
		assert.equal(
			wallpaperPaletteFromPixels(
				pixels(
					{ color: [255, 0, 0, 0], count: 100 },
					{ color: [255, 255, 255, 255], count: 100 },
					{ color: [0, 0, 0, 255], count: 100 },
					{ color: [120, 120, 120, 255], count: 100 }
				)
			),
			null
		);
		assert.equal(wallpaperPaletteFromPixels(new Uint8ClampedArray()), null);
	});

	it('keeps controls colorful while giving text and filled buttons readable tones', () => {
		for (const color of [
			[30, 80, 180, 255],
			[190, 50, 25, 255],
			[40, 150, 60, 255],
			[220, 180, 30, 255]
		]) {
			const palette = wallpaperPaletteFromPixels(
				new Uint8ClampedArray(Array.from({ length: 100 }, () => color).flat())
			);
			assert.ok(palette);
			const base = Hct.fromInt(argbFromHex(palette.color));
			assert.ok(base.chroma >= 30, `accent is colorful: ${palette.color}`);
			assert.ok(Math.abs(base.tone - 65) < 1);
			assert.ok(
				(luminance(parseHexColor(palette.textColor)) + 0.05) /
					(luminance([104, 104, 104]) + 0.05) >=
					4.5
			);
			assert.ok(1.05 / (luminance(parseHexColor(solidAccentColor(palette.color))) + 0.05) >= 4.5);
			assert.ok(
				(luminance(parseHexColor(palette.iconColor)) + 0.05) /
					(luminance([104, 104, 104]) + 0.05) >=
					3
			);
			assert.notEqual(palette.color, palette.textColor);
		}
	});

	it('validates the worker result at its boundary', () => {
		assert.deepEqual(
			parseWallpaperPalette({ color: '#3344aa', textColor: '#bbccff', iconColor: '#8899ee' }),
			{
				color: '#3344aa',
				textColor: '#bbccff',
				iconColor: '#8899ee'
			}
		);
		for (const value of [
			null,
			[],
			'blue',
			{ color: 'url(example)', textColor: '#ffffff' },
			{ color: '#123456' }
		]) {
			assert.equal(parseWallpaperPalette(value), null);
		}
	});

	it('returns the same palette for an unchanged image', () => {
		const gradient = new Uint8ClampedArray(
			Array.from({ length: 400 }, (_, i) => [
				40 + (i % 150),
				80 + (i % 120),
				140 + (i % 110),
				255
			]).flat()
		);
		const expected = wallpaperPaletteFromPixels(gradient);
		assert.ok(expected);
		for (let repeat = 0; repeat < 5; repeat++) {
			assert.deepEqual(wallpaperPaletteFromPixels(gradient), expected);
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
