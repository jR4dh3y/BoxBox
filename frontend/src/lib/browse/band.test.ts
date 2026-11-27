import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { edgeScrollStep, itemAt, itemsInBand } from './band.js';
import { gridMetrics } from './zoom.js';

const at = (x: number, y: number) => ({ x, y });

describe('rubber band', () => {
	test('a list band takes every row it crosses', () => {
		const list = { kind: 'list', rowHeight: 32 } as const;
		assert.deepEqual(itemsInBand(list, at(10, 40), at(300, 100), 10), [1, 2, 3]);
		// Below the last row there is nothing to take.
		assert.deepEqual(itemsInBand(list, at(10, 300), at(10, 400), 5), []);
		assert.equal(itemAt(list, at(5, 33), 10), 1);
		assert.equal(itemAt(list, at(5, 33), 1), null);
	});

	test('a grid band skips the gaps between tiles', () => {
		const metrics = gridMetrics(1);
		const grid = { kind: 'grid', metrics, columns: 3 } as const;
		// The gap after the first tile is empty space; the tile itself is not.
		const gapX = metrics.padding + metrics.tileWidth + metrics.gap / 2;
		assert.equal(itemAt(grid, at(gapX, 50), 9), null);
		assert.equal(itemAt(grid, at(metrics.padding + 5, metrics.padding + 5), 9), 0);
		// From the first tile across to the third, one row down.
		const across = itemsInBand(grid, at(metrics.padding + 5, metrics.padding + 5), at(400, 200), 9);
		assert.deepEqual(across, [0, 1, 2, 3, 4, 5]);
	});

	test('a held drag scrolls toward the pointer, a step at most', () => {
		assert.equal(edgeScrollStep(50, 100, 500), -24);
		assert.equal(edgeScrollStep(510, 100, 500), 10);
		assert.equal(edgeScrollStep(300, 100, 500), 0);
	});
});
