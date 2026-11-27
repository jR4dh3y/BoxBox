import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { FileInfo } from '#lib/api/files.js';
import { ViewCursor } from './viewCursor.svelte.js';

const file = (name: string): FileInfo => ({
	name,
	path: `home/${name}`,
	size: 0,
	isDir: false,
	modTime: '',
	permissions: ''
});
const plain = { ctrlKey: false, metaKey: false, shiftKey: false };
const shift = { ...plain, shiftKey: true };

test('a Shift range starts from the anchored item after the view is re-sorted', () => {
	let items = ['a', 'b', 'c', 'd'].map(file);
	let selected: ReadonlySet<string> = new Set();
	const cursor = new ViewCursor(() => ({
		items,
		selectedPaths: selected,
		onSelectionChange: (paths) => (selected = paths)
	}));
	cursor.click(items[1]!, plain);
	items = [...items].reverse();
	cursor.click(items[0]!, shift);
	// d, c, b: from the new top down to b, where the anchor is now.
	assert.deepEqual([...selected].sort(), ['home/b', 'home/c', 'home/d']);
});
