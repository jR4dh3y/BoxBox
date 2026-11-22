import assert from 'node:assert/strict';
import { test } from 'node:test';
import { getFileShortcut } from './fileShortcuts';
const key = (key: string, overrides = {}) => ({
	key,
	ctrlKey: false,
	metaKey: false,
	altKey: false,
	shiftKey: false,
	...overrides
});
test('page shortcuts map Ctrl and Cmd alike and leave Alt and plain keys alone', () => {
	assert.equal(getFileShortcut(key('a', { ctrlKey: true })), 'select-all');
	assert.equal(getFileShortcut(key('S', { metaKey: true })), 'select-pattern');
	assert.equal(getFileShortcut(key('i', { ctrlKey: true })), 'invert-selection');
	assert.equal(getFileShortcut(key('n', { ctrlKey: true, shiftKey: true })), 'new-folder');
	assert.equal(getFileShortcut(key('v', { ctrlKey: true })), 'paste');
	assert.equal(getFileShortcut(key('c', { ctrlKey: true, altKey: true })), null);
	assert.equal(getFileShortcut(key('F1')), 'shortcuts');
	assert.equal(getFileShortcut(key('F11')), 'details');
	assert.equal(getFileShortcut(key('Escape')), 'clear-selection');
	assert.equal(getFileShortcut(key('a')), null);
});
test('zoom takes plus, equals, minus and zero', () => {
	assert.equal(getFileShortcut(key('+', { ctrlKey: true, shiftKey: true })), 'zoom-in');
	assert.equal(getFileShortcut(key('=', { ctrlKey: true })), 'zoom-in');
	assert.equal(getFileShortcut(key('-', { ctrlKey: true })), 'zoom-out');
	assert.equal(getFileShortcut(key('0', { ctrlKey: true })), 'zoom-reset');
});
