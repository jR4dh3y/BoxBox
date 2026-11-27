import assert from 'node:assert/strict';
import { test } from 'node:test';
import { getFileContextMenuItems } from '../utils/fileContextMenu';

test('read-only menus disable mutations while keeping copy and inspection available', () => {
	const items = [
		{
			name: 'notes.txt',
			path: 'media/notes.txt',
			isDir: false,
			size: 1,
			modTime: '',
			permissions: ''
		}
	];
	for (const selection of [[], items]) {
		const menu = getFileContextMenuItems({
			items: selection,
			canPaste: true,
			canCreate: true,
			readOnly: true
		});
		for (const action of ['new-file', 'new-folder', 'cut', 'paste', 'rename', 'delete']) {
			const item = menu.find((item) => item.id === action);
			if (item) assert.equal(item.disabled, true, `${action} must not offer a read-only mutation`);
		}
		for (const action of ['copy', 'properties']) {
			const item = menu.find((item) => item.id === action);
			if (item) assert.notEqual(item.disabled, true);
		}
	}
});
