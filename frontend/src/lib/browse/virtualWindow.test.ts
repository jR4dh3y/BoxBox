import assert from 'node:assert/strict';
import { test } from 'node:test';
import { columnsFor, revealOffset, visibleRange } from './virtualWindow';

const list = { count: 10_000, columns: 1, rowHeight: 40 };

test('a list renders only the rows in view plus the overscan', () => {
	const range = visibleRange(list, { scrollTop: 0, height: 400 }, 5);
	assert.deepEqual(range, { start: 0, end: 15, before: 0, after: (10_000 - 15) * 40 });
});

test('spacers keep the full scroll height wherever the window is', () => {
	for (const scrollTop of [0, 1234, 200_000, 399_600]) {
		const range = visibleRange(list, { scrollTop, height: 400 }, 5);
		assert.equal(range.before + (range.end - range.start) * 40 + range.after, 10_000 * 40);
		assert.ok(range.end - range.start <= 21, `rendered ${range.end - range.start} rows`);
	}
});

test('the last rows render at the bottom of the list', () => {
	const range = visibleRange(list, { scrollTop: 10_000 * 40 - 400, height: 400 }, 5);
	assert.equal(range.end, 10_000);
	assert.equal(range.after, 0);
});

test('a list that shrank below the scroll position still renders its last rows', () => {
	const range = visibleRange({ ...list, count: 3 }, { scrollTop: 50_000, height: 400 }, 5);
	assert.deepEqual([range.start, range.end, range.after], [0, 3, 0]);
});

test('grid windows start and end on whole rows', () => {
	const grid = { count: 1003, columns: 7, rowHeight: 141 };
	const range = visibleRange(grid, { scrollTop: 141 * 50 + 20, height: 600 }, 2);
	assert.equal(range.start % 7, 0);
	assert.equal(range.before, (range.start / 7) * 141);
	const last = visibleRange(grid, { scrollTop: 141 * 144, height: 600 }, 2);
	assert.equal(last.end, 1003);
});

test('an empty collection renders nothing', () => {
	assert.deepEqual(visibleRange({ ...list, count: 0 }, { scrollTop: 0, height: 400 }, 5), {
		start: 0,
		end: 0,
		before: 0,
		after: 0
	});
});

test('columns follow the width, with at least one', () => {
	assert.equal(columnsFor(1000, 112, 12), 8);
	assert.equal(columnsFor(111, 112, 12), 1);
	assert.equal(columnsFor(0, 112, 12), 1);
});

test('reveal scrolls only as far as needed to show the whole item', () => {
	const viewport = { scrollTop: 400, height: 400 };
	assert.equal(revealOffset(list, viewport, 12), null);
	assert.equal(revealOffset(list, viewport, 3), 120);
	assert.equal(revealOffset(list, viewport, 25), 26 * 40 - 400);
	assert.equal(revealOffset(list, viewport, 9_999), 10_000 * 40 - 400);
	assert.equal(revealOffset({ count: 100, columns: 5, rowHeight: 100 }, viewport, 47), 1000 - 400);
});
