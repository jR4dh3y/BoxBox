import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { canDropInto, dropOperation, readDroppedLocalFiles } from './fileDrag';

const folder = { path: 'home/photos', name: 'photos', isDir: true };
const file = { path: 'home/notes.txt', name: 'notes.txt', isDir: false };

describe('file drag and drop', () => {
	test('BoxBox items move unless Ctrl is held; other apps always copy', () => {
		assert.equal(dropOperation('files', { ctrlKey: false }), 'move');
		assert.equal(dropOperation('files', { ctrlKey: true }), 'copy');
		assert.equal(dropOperation('external', { ctrlKey: false }), 'copy');
	});

	test('refuses a folder dropped into itself or its own subfolder', () => {
		assert.equal(canDropInto([folder], 'home/photos', 'copy'), false);
		assert.equal(canDropInto([folder], 'home/photos/2026', 'move'), false);
		assert.equal(canDropInto([folder], 'home/photos-old', 'move'), true);
	});

	test('a move to the folder the items are already in does nothing, a copy keeps both', () => {
		assert.equal(canDropInto([file, folder], 'home', 'move'), false);
		assert.equal(canDropInto([file, folder], 'home', 'copy'), true);
		assert.equal(canDropInto([file], 'home/photos', 'move'), true);
	});

	test('accepts drags from another window, whose items are checked on drop', () => {
		assert.equal(canDropInto(null, 'home', 'move'), true);
		assert.equal(canDropInto(null, '', 'move'), false);
	});

	test('reads local files from a URI list as BoxBox paths', () => {
		assert.deepEqual(
			readDroppedLocalFiles(
				'# from a file manager\r\nfile:///tmp/a%20b.txt\r\nhttps://example.test/x\r\nfile:///\r\nfile://other-host/c'
			),
			[{ path: 'root/tmp/a b.txt', name: 'a b.txt', isDir: false }]
		);
	});
});

const remoteMount = '@remote/12345678-1234-4234-8234-123456789abc/media';
test('cross-source drags copy by default while same-source drags can move', () => {
	const source = [{ path: 'home/a.txt', name: 'a.txt', isDir: false }];
	assert.equal(dropOperation('files', { ctrlKey: false }, remoteMount, source), 'copy');
	assert.equal(dropOperation('files', { ctrlKey: false }, 'home/dest', source), 'move');
});
