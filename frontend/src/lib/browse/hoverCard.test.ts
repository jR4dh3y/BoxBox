import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import type { FileInfo } from '#lib/api/files.js';
import {
	HoverCard,
	describeTotals,
	folderTotals,
	type HoverCardState
} from './hoverCard.svelte.js';

const file = (name: string, size = 10, isDir = false): FileInfo => ({
	name,
	path: `media/${name}`,
	size,
	isDir,
	modTime: '2026-01-01T00:00:00Z',
	permissions: 'rw-r--r--'
});
const pointer = () => ({ x: 5, y: 7, touch: false });
const tick = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe('hover card', () => {
	test('counts a folder one level down', () => {
		const totals = folderTotals([file('a'), file('b', 5), file('sub', 4096, true)]);
		assert.deepEqual(totals, { files: 2, folders: 1, bytes: 15 });
		assert.equal(describeTotals(totals), '1 folder, 2 files');
		assert.equal(describeTotals({ files: 1, folders: 0, bytes: 0 }), '1 file');
	});

	test('shows after the pointer rests, and counts a folder', async () => {
		const card = new HoverCard({
			enabled: () => true,
			location: () => null,
			listFolder: async () => [file('a')],
			delayMs: 10
		});
		// Read through a function, so each check sees the state as it is then.
		const state = (): HoverCardState => card.state;
		card.rest(file('dir', 0, true), pointer());
		assert.equal(state().kind, 'waiting');
		await tick(20);
		assert.equal(state().kind, 'shown');
		await tick(0);
		const shown = state();
		assert.deepEqual(shown.kind === 'shown' && shown.contents, {
			kind: 'ready',
			totals: { files: 1, folders: 0, bytes: 10 }
		});
	});

	test('a pass across items, a press or Settings show nothing', async () => {
		let enabled = true;
		const card = new HoverCard({
			enabled: () => enabled,
			location: () => null,
			listFolder: async () => [],
			delayMs: 10
		});
		card.rest(file('a'), pointer());
		card.hide();
		await tick(20);
		assert.equal(card.state.kind, 'hidden');
		enabled = false;
		card.rest(file('a'), pointer());
		await tick(20);
		assert.equal(card.state.kind, 'hidden');
	});
});
