import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import type { DriveStats } from '#lib/api/files.js';
import { DriveHover, cardPlacement, type DriveHoverState } from './hoverCard.svelte.js';

const media: DriveStats = {
	name: 'media',
	path: 'media',
	mountPoint: '/mnt/media',
	totalBytes: 100,
	freeBytes: 40,
	usedBytes: 60,
	usedPct: 60,
	readOnly: false
};
const pointer = () => ({ x: 5, y: 7, touch: false });
const tick = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe('drive hover card', () => {
	test('shows the rested-on drive once its stats arrive', async () => {
		const card = new DriveHover({ enabled: () => true, stats: async () => [media], delayMs: 10 });
		const state = (): DriveHoverState => card.state;
		card.rest('media', 'Media', pointer());
		assert.equal(state().kind, 'waiting');
		await tick(30);
		const shown = state();
		assert.equal(shown.kind, 'shown');
		if (shown.kind !== 'shown') return;
		assert.deepEqual(shown.usage, { kind: 'ready', drive: media });
		assert.equal(shown.name, 'Media');
	});

	test('a drive missing from the stats, or a failed read, says so', async () => {
		const missing = new DriveHover({ enabled: () => true, stats: async () => [], delayMs: 1 });
		missing.rest('gone', 'Gone', pointer());
		const failed = new DriveHover({
			enabled: () => true,
			stats: () => Promise.reject(new Error('offline')),
			delayMs: 1
		});
		failed.rest('media', 'Media', pointer());
		await tick(20);
		for (const card of [missing, failed]) {
			const state: DriveHoverState = card.state;
			assert.equal(state.kind === 'shown' && state.usage.kind, 'failed');
		}
	});

	test('leaving, touch and the setting keep it hidden', async () => {
		const card = new DriveHover({ enabled: () => true, stats: async () => [media], delayMs: 5 });
		card.rest('media', 'Media', pointer());
		card.hide();
		card.rest('media', 'Media', { x: 0, y: 0, touch: true });
		const off = new DriveHover({ enabled: () => false, stats: async () => [media], delayMs: 5 });
		off.rest('media', 'Media', pointer());
		await tick(20);
		assert.equal(card.state.kind, 'hidden');
		assert.equal(off.state.kind, 'hidden');
	});
});

describe('card placement', () => {
	const viewport = { width: 800, height: 600 };
	test('sits below and right of the pointer', () => {
		assert.deepEqual(cardPlacement({ x: 100, y: 100 }, 280, 120, viewport), {
			left: 112,
			top: 120
		});
	});
	test('stays inside the window at its edges', () => {
		assert.deepEqual(cardPlacement({ x: 700, y: 550 }, 280, 120, viewport), {
			left: 512,
			top: 422
		});
	});
});
