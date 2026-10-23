import assert from 'node:assert/strict';
import { afterEach, beforeEach, test } from 'node:test';
import type { FileInfo } from '#lib/api/files.js';
import {
	useFileCollection,
	type FileCollectionView,
	type FileKeyInput
} from './fileCollection.svelte';

function entry(index: number): FileInfo {
	const name = `file-${index}.txt`;
	return { name, path: `media/${name}`, isDir: false, size: 0, modTime: '', permissions: '' };
}

// Item navigation checks whether the key came from a text field.
const realInput = globalThis.HTMLInputElement;
beforeEach(() => Object.assign(globalThis, { HTMLInputElement: class {} }));
afterEach(() => Object.assign(globalThis, { HTMLInputElement: realInput }));

function key(
	key: string,
	modifiers: Partial<Pick<FileKeyInput, 'shiftKey' | 'ctrlKey'>> = {}
): FileKeyInput & { prevented: boolean } {
	return {
		key,
		altKey: false,
		ctrlKey: false,
		metaKey: false,
		shiftKey: false,
		target: null,
		prevented: false,
		preventDefault() {
			this.prevented = true;
		},
		...modifiers
	};
}

function setup(columns: number) {
	const items = Array.from({ length: 10_000 }, (_, index) => entry(index));
	const focused: number[] = [];
	let selected = new Set<string>();
	const view: FileCollectionView & { focusedPath: string | null } = {
		columns,
		focusedPath: null,
		focus: async (index) => {
			focused.push(index);
			view.focusedPath = items[index].path;
		}
	};
	const collection = useFileCollection(
		() => ({
			items,
			selectedPaths: selected,
			fileOpenBehavior: 'double-click',
			onSelectionChange: (paths) => (selected = paths)
		}),
		view
	);
	return { items, focused, view, collection, selected: () => [...selected] };
}

test('End reaches the last item even though the view never rendered it', () => {
	const { items, focused, collection, selected } = setup(1);
	const event = key('End');
	collection.keydown(items[0], event);
	assert.equal(event.prevented, true);
	assert.deepEqual(focused, [9_999]);
	assert.deepEqual(selected(), ['media/file-9999.txt']);
});

test('arrow keys move a whole grid row using the view columns', () => {
	const { items, focused, collection } = setup(7);
	collection.keydown(items[20], key('ArrowDown'));
	collection.keydown(items[20], key('ArrowUp'));
	collection.keydown(items[20], key('ArrowRight'));
	assert.deepEqual(focused, [27, 13, 21]);
});

test('Shift extends a range from the anchor across unrendered items', () => {
	const { items, collection, selected } = setup(1);
	collection.keydown(items[5], key(' '));
	collection.keydown(items[5], key('End', { shiftKey: true }));
	assert.equal(selected().length, 9_995);
	assert.equal(selected()[0], 'media/file-5.txt');
});

test('Ctrl moves focus without changing the selection', () => {
	const { items, focused, collection, selected } = setup(1);
	collection.keydown(items[3], key('ArrowDown', { ctrlKey: true }));
	assert.deepEqual(focused, [4]);
	assert.deepEqual(selected(), []);
});

test('after the focused item scrolls out, keys on the scroller continue from it', () => {
	const { items, focused, collection } = setup(1);
	collection.keydown(items[100], key('ArrowDown'));
	const scroller = new EventTarget();
	collection.scrollerKeydown({ ...key('ArrowDown'), target: scroller, currentTarget: scroller });
	collection.scrollerKeydown({
		...key('ArrowUp'),
		target: new EventTarget(),
		currentTarget: scroller
	});
	assert.deepEqual(focused, [101, 102]);
});

test('an item reached with Tab is where scroller keys resume', () => {
	const { items, focused, view, collection } = setup(9);
	view.focusedPath = items[10].path; // Tab moved focus here; no key was pressed on this item.
	const scroller = new EventTarget();
	collection.scrollerKeydown({ ...key('ArrowDown'), target: scroller, currentTarget: scroller });
	assert.deepEqual(focused, [19]);
});

test('after a re-sort, scroller keys resume from the focused item at its new position', () => {
	const { items, focused, view, collection } = setup(1);
	view.focusedPath = items[11].path;
	items.reverse();
	const scroller = new EventTarget();
	collection.scrollerKeydown({ ...key('ArrowDown'), target: scroller, currentTarget: scroller });
	assert.deepEqual(focused, [items.findIndex((item) => item.path === 'media/file-11.txt') + 1]);
});

test('when the remembered item is gone, a navigation key starts at the first item', () => {
	const { focused, view, collection } = setup(9);
	view.focusedPath = 'media/deleted.txt';
	const scroller = new EventTarget();
	const event = { ...key('ArrowDown'), target: scroller, currentTarget: scroller };
	collection.scrollerKeydown(event);
	assert.deepEqual(focused, [0]);
	assert.equal(event.prevented, true);
});
