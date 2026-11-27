import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { parseWallpaperPalette } from './wallpaperPaletteProtocol.js';

describe('wallpaper accent', () => {
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
			{ color: 'url(example)', textColor: '#ffffff', iconColor: '#ffffff' },
			{ color: '#123456', textColor: 'url(example)', iconColor: '#ffffff' },
			{ color: '#123456', textColor: '#ffffff', iconColor: 'url(example)' },
			{ color: '#123456' }
		]) {
			assert.equal(parseWallpaperPalette(value), null);
		}
	});
});
