import assert from 'node:assert/strict';
import { test } from 'node:test';
import { detailsContent, summarizeSelection } from './details';

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

test('the Details panel shows the folder, one item, or a count of several', () => {
	const item = (name: string, isDir = false, size = 10) => ({
		path: `home/${name}`,
		name,
		modTime: '',
		permissions: '',
		size,
		isDir
	});
	assert.deepEqual(detailsContent([], 7), { kind: 'folder', count: 7 });
	assert.equal(detailsContent([item('a')], 7).kind, 'single');
	const many = detailsContent([item('a'), item('b', true, 4096), item('c')], 7);
	assert.deepEqual(many, {
		kind: 'many',
		files: 2,
		folders: 1,
		bytes: 20,
		names: ['a', 'b', 'c'],
		more: 0
	});
	const lots = detailsContent(
		Array.from({ length: 60 }, (_, index) => item(`f${index}`)),
		60
	);
	assert.ok(lots.kind === 'many' && lots.names.length === 50 && lots.more === 10);
});
