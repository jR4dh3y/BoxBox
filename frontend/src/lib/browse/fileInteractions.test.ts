import assert from 'node:assert/strict';
import test from 'node:test';
import {
	applyFileSelection,
	fileSelectionGestureFromModifiers,
	getSelectedContextMenuItems,
	shouldOpenOnClick,
	shouldOpenOnDoubleClick
} from './fileInteractions';
import { normalizeFileOpenBehavior } from '../utils/fileOpenBehavior';

const orderedPaths = ['a', 'b', 'c', 'd', 'e'];

test('right-clicking an unselected item uses only it for the context menu', () => {
	const items = orderedPaths.map((path) => ({ path }));
	const menuItems = getSelectedContextMenuItems(items, new Set(['a', 'b']), items[3]);

	assert.deepEqual(menuItems, [items[3]]);
});

test('right-clicking a selected item keeps the visible selection for the context menu', () => {
	const items = orderedPaths.map((path) => ({ path }));
	const menuItems = getSelectedContextMenuItems(items, new Set(['a', 'd']), items[3]);

	assert.deepEqual(menuItems, [items[0], items[3]]);
});

test('Shift ranges keep the original anchor when the endpoint moves', () => {
	const first = applyFileSelection(orderedPaths, new Set(), null, 'b', { type: 'replace' });
	const forward = applyFileSelection(
		orderedPaths,
		first.paths,
		first.anchorPath,
		'd',
		fileSelectionGestureFromModifiers({ ctrlKey: false, metaKey: false, shiftKey: true })
	);
	const backward = applyFileSelection(
		orderedPaths,
		forward.paths,
		forward.anchorPath,
		'a',
		fileSelectionGestureFromModifiers({ ctrlKey: false, metaKey: false, shiftKey: true })
	);

	assert.deepEqual([...forward.paths], ['b', 'c', 'd']);
	assert.deepEqual([...backward.paths], ['a', 'b']);
	assert.equal(backward.anchorPath, 'b');
});

test('Ctrl/Cmd toggles one item and Ctrl/Cmd+Shift adds a stable range', () => {
	const anchor = applyFileSelection(orderedPaths, new Set(['a']), null, 'b', {
		type: 'toggle'
	});
	const commandToggle = fileSelectionGestureFromModifiers({
		ctrlKey: false,
		metaKey: true,
		shiftKey: false
	});
	const toggled = applyFileSelection(
		orderedPaths,
		anchor.paths,
		anchor.anchorPath,
		'b',
		commandToggle
	);
	const addedRange = applyFileSelection(
		orderedPaths,
		toggled.paths,
		toggled.anchorPath,
		'd',
		fileSelectionGestureFromModifiers({ ctrlKey: true, metaKey: false, shiftKey: true })
	);

	assert.deepEqual([...toggled.paths], ['a']);
	assert.deepEqual([...addedRange.paths], ['a', 'b', 'c', 'd']);
	assert.equal(addedRange.anchorPath, 'b');
});

test('checkbox state sets or removes an item without disturbing other selections', () => {
	const selected = applyFileSelection(orderedPaths, new Set(['a', 'c']), 'a', 'b', {
		type: 'set',
		selected: true
	});
	const removed = applyFileSelection(orderedPaths, selected.paths, selected.anchorPath, 'c', {
		type: 'set',
		selected: false
	});

	assert.deepEqual([...selected.paths], ['a', 'c', 'b']);
	assert.deepEqual([...removed.paths], ['a', 'b']);
	assert.equal(removed.anchorPath, 'c');
});

test('a missing anchor resets the next range at its target', () => {
	const result = applyFileSelection(orderedPaths, new Set(['e']), 'missing', 'c', {
		type: 'range'
	});

	assert.deepEqual([...result.paths], ['c']);
	assert.equal(result.anchorPath, 'c');
});

test('single-click activation migrates legacy preference and ignores modified or second clicks', () => {
	assert.equal(normalizeFileOpenBehavior(undefined, true), 'single-click');
	assert.equal(normalizeFileOpenBehavior(undefined, false), 'double-click');
	assert.equal(normalizeFileOpenBehavior('single-click', false), 'single-click');
	assert.equal(normalizeFileOpenBehavior('invalid', true), 'single-click');
	assert.equal(
		shouldOpenOnClick('single-click', {
			ctrlKey: false,
			metaKey: false,
			shiftKey: false,
			detail: 1
		}),
		true
	);
	assert.equal(
		shouldOpenOnClick('single-click', {
			ctrlKey: false,
			metaKey: false,
			shiftKey: false,
			detail: 2
		}),
		false
	);
	assert.equal(
		shouldOpenOnClick('single-click', {
			ctrlKey: true,
			metaKey: false,
			shiftKey: false,
			detail: 1
		}),
		false
	);
});

test('double-click activation ignores modifier selections', () => {
	assert.equal(
		shouldOpenOnDoubleClick('double-click', {
			ctrlKey: false,
			metaKey: false,
			shiftKey: false
		}),
		true
	);
	assert.equal(
		shouldOpenOnDoubleClick('double-click', {
			ctrlKey: false,
			metaKey: true,
			shiftKey: false
		}),
		false
	);
	assert.equal(
		shouldOpenOnDoubleClick('single-click', {
			ctrlKey: false,
			metaKey: false,
			shiftKey: false
		}),
		false
	);
});
