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
