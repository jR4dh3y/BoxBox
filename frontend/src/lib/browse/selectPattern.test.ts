import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { invertedPaths, matchesPattern, pathsMatching } from './selectPattern.js';

describe('select by pattern', () => {
	test('a wildcard pattern matches the whole name, ignoring case', () => {
		assert.equal(matchesPattern('*.jpg', 'Holiday.JPG'), true);
		assert.equal(matchesPattern('*.jpg', 'holiday.jpg.txt'), false);
		assert.equal(matchesPattern('img_??.png', 'IMG_01.png'), true);
		assert.equal(matchesPattern('img_??.png', 'IMG_001.png'), false);
		assert.equal(matchesPattern('a*b*c', 'axxbyyc'), true);
		assert.equal(matchesPattern('*', 'anything'), true);
		assert.equal(matchesPattern('*', '*notes.txt'), true);
		assert.equal(matchesPattern('*.txt', '*notes.txt'), true);
	});

	test('plain text matches anywhere; an empty pattern matches nothing', () => {
		assert.equal(matchesPattern('report', 'Q3 Report final.pdf'), true);
		assert.equal(matchesPattern('  ', 'x'), false);
	});

	test('picks the matches, and inversion swaps picked and not', () => {
		const items = [
			{ name: 'a.jpg', path: 'm/a.jpg' },
			{ name: 'b.txt', path: 'm/b.txt' }
		];
		assert.deepEqual([...pathsMatching(items, '*.jpg')], ['m/a.jpg']);
		assert.deepEqual([...invertedPaths(items, new Set(['m/a.jpg']))], ['m/b.txt']);
	});
});
