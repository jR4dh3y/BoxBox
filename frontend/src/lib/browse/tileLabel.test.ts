import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { tileLabelLines, type Measure } from './tileLabel.js';

/** Every character one pixel wide. */
const mono: Measure = (text) => {
	const starts = Array.from(text).map((_, index): [number, number] => [index, index]);
	return { starts, width: starts.length };
};

describe('tile label', () => {
	test('a short name is one line', () => {
		assert.deepEqual(tileLabelLines('soil.hash', 12, mono), ['soil.hash', null]);
	});

	test('the first line ends at a word', () => {
		assert.deepEqual(tileLabelLines('Soil Mechanics', 12, mono), ['Soil', 'Mechanics']);
	});

	test('a long second line keeps its end', () => {
		const [first, second] = tileLabelLines(
			'Soil Mechanics CivilEnggForAll GATE Study Material (1).pdf',
			20,
			mono
		);
		assert.equal(first, 'Soil Mechanics');
		assert.ok(second);
		assert.ok(Array.from(second).length <= 20, second);
		assert.ok(second.startsWith('CivilEngg'), second);
		assert.ok(second.endsWith('(1).pdf'), second);
		assert.ok(second.includes('…'));
	});

	test('one long word breaks where it overflows', () => {
		assert.deepEqual(tileLabelLines('abcdefghijklmnopqrstuvwxyz', 10, mono), [
			'abcdefghij',
			'klmn…wxyz'
		]);
	});

	test('a name breaks after a separator', () => {
		assert.equal(tileLabelLines('holiday_photos_2024.zip', 16, mono)[0], 'holiday_photos_');
	});
});
