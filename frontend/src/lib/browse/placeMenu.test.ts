import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { foldCrumbs } from './foldCrumbs.js';
import { placeMenuItems } from './placeMenu.js';

describe('place menu', () => {
	test('offers a tab, paste and the path, and what the place kind adds', () => {
		const ids = (items: { id: string }[]) => items.map((item) => item.id);
		assert.deepEqual(ids(placeMenuItems({ kind: 'folder', path: 'home/a' }, true)), [
			'new-tab',
			'paste',
			'copy-path'
		]);
		assert.ok(ids(placeMenuItems({ kind: 'drive', path: 'home' }, true)).includes('rename-drive'));
		assert.ok(ids(placeMenuItems({ kind: 'favorite', path: 'home/a' }, true)).includes('unpin'));
		assert.ok(
			!ids(placeMenuItems({ kind: 'place', path: 'home/Downloads' }, true)).includes(
				'rename-drive'
			),
			'a place keeps its folder name'
		);
		const root = placeMenuItems({ kind: 'drive', path: '' }, true);
		assert.equal(root.find((item) => item.id === 'paste')?.disabled, true);
		assert.ok(!ids(root).includes('rename-drive'));
	});

	test('long paths fold their middle crumbs', () => {
		assert.deepEqual(foldCrumbs(['a', 'b', 'c']), { shown: ['a', 'b', 'c'], folded: [] });
		assert.deepEqual(foldCrumbs(['a', 'b', 'c', 'd', 'e', 'f']), {
			shown: ['a', 'd', 'e', 'f'],
			folded: ['b', 'c']
		});
	});
});
