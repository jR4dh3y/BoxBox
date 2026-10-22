import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { getBrowseShortcut, type BrowseShortcutInput } from './paneShortcuts';

function input(overrides: Partial<BrowseShortcutInput> = {}): BrowseShortcutInput {
	return {
		key: '',
		ctrlKey: false,
		metaKey: false,
		shiftKey: false,
		altKey: false,
		editable: false,
		...overrides
	};
}

describe('browse keyboard shortcuts', () => {
	test('maps tab, split, and navigation shortcuts', () => {
		assert.equal(getBrowseShortcut(input({ key: 't', ctrlKey: true })), 'new-tab');
		assert.equal(getBrowseShortcut(input({ key: 'w', metaKey: true })), 'close-tab');
		assert.equal(getBrowseShortcut(input({ key: 'Tab', ctrlKey: true })), 'next-tab');
		assert.equal(
			getBrowseShortcut(input({ key: 'Tab', ctrlKey: true, shiftKey: true })),
			'previous-tab'
		);
		assert.equal(getBrowseShortcut(input({ key: 'F3' })), 'toggle-split');
		assert.equal(getBrowseShortcut(input({ key: 'ArrowLeft', altKey: true })), 'back');
		assert.equal(getBrowseShortcut(input({ key: 'ArrowRight', altKey: true })), 'forward');
		assert.equal(getBrowseShortcut(input({ key: 'ArrowUp', altKey: true })), 'up');
		assert.equal(getBrowseShortcut(input({ key: 'F5' })), 'refresh');
	});

	test('hidden, location, and editable-target behavior is explicit', () => {
		assert.equal(getBrowseShortcut(input({ key: 'h', ctrlKey: true })), 'toggle-hidden');
		assert.equal(
			getBrowseShortcut(input({ key: 'l', ctrlKey: true, editable: true })),
			'focus-location'
		);
		assert.equal(getBrowseShortcut(input({ key: 'h', ctrlKey: true, editable: true })), null);
		assert.equal(
			getBrowseShortcut(input({ key: 'ArrowLeft', altKey: true, editable: true })),
			null
		);
		assert.equal(getBrowseShortcut(input({ key: 'z', ctrlKey: true })), null);
		assert.equal(getBrowseShortcut(input({ key: 'v', ctrlKey: true })), null);
		assert.equal(getBrowseShortcut(input({ key: 'v', metaKey: true, editable: true })), null);
	});
});
