import assert from 'node:assert/strict';
import { test } from 'node:test';
import { summarizeSelection, parentFolder, fullFileDate } from './details';

test('selection totals exclude directory metadata sizes', () => {
	const base = { path: 'home/file', name: 'file', modTime: '', permissions: '' };
	assert.deepEqual(
		summarizeSelection([
			{ ...base, size: 4096, isDir: true },
			{ ...base, size: 512, isDir: false },
			{ ...base, size: 1024, isDir: false }
		]),
		{ count: 3, folders: 1, files: 2, bytes: 1536 }
	);
	assert.deepEqual(summarizeSelection([]), { count: 0, folders: 0, files: 0, bytes: 0 });
});

test('details handle root locations and unknown timestamps', () => {
	assert.equal(parentFolder('home/a/file.txt'), 'home/a');
	assert.equal(parentFolder('home'), '');
	assert.equal(fullFileDate('invalid'), 'Unknown');
});
