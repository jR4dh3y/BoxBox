import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fileNavigationIndex, getFileShortcut } from './fileShortcuts';
const key = (key: string, overrides = {}) => ({
	key,
	ctrlKey: false,
	metaKey: false,
	altKey: false,
	shiftKey: false,
	...overrides
});
test('file shortcuts distinguish destructive actions and preserve modified keys', () => {
	assert.equal(getFileShortcut(key('Delete')), 'delete');
	assert.equal(getFileShortcut(key('Delete', { shiftKey: true })), 'delete-permanently');
	assert.equal(getFileShortcut(key('n', { ctrlKey: true, shiftKey: true })), 'new-folder');
	assert.equal(getFileShortcut(key('Enter', { altKey: true })), 'details');
	assert.equal(getFileShortcut(key('c', { metaKey: true })), 'copy');
	assert.equal(getFileShortcut(key('v', { ctrlKey: true })), 'paste');
	assert.equal(getFileShortcut(key('c', { ctrlKey: true, altKey: true })), null);
	assert.equal(getFileShortcut(key('f', { ctrlKey: true })), 'search');
});
test('navigation follows grid columns and clamps at page boundaries', () => {
	assert.equal(fileNavigationIndex('ArrowDown', 1, 8, 3), 4);
	assert.equal(fileNavigationIndex('ArrowUp', 1, 8, 3), 0);
	assert.equal(fileNavigationIndex('ArrowRight', 7, 8, 3), 7);
	assert.equal(fileNavigationIndex('End', 1, 8, 3), 7);
	assert.equal(fileNavigationIndex('Home', 5, 8, 3), 0);
	assert.equal(fileNavigationIndex('Enter', 0, 8), null);
	assert.equal(fileNavigationIndex('Home', 0, 0), null);
});
