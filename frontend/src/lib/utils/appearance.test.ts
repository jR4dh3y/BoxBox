import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { solidAccentColor } from '../theme/colors.js';

function contrastWithWhite(hex: string): number {
	const channels = [1, 3, 5].map((start) => {
		const value = Number.parseInt(hex.slice(start, start + 2), 16) / 255;
		return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
	});
	const [r = 0, g = 0, b = 0] = channels;
	return 1.05 / (0.2126 * r + 0.7152 * g + 0.0722 * b + 0.05);
}

describe('solidAccentColor', () => {
	it('keeps white text readable on any accent', () => {
		for (const accent of ['#4a9eff', '#ffffff', '#ffd400', '#10b981', '#ff3b30', '#000000']) {
			assert.ok(contrastWithWhite(solidAccentColor(accent)) >= 4.5, accent);
		}
	});

	it('leaves an accent that already has enough contrast unchanged', () => {
		assert.equal(solidAccentColor('#1d4ed8'), '#1d4ed8');
	});
});
