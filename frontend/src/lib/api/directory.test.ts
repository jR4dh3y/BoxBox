import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseDirectory } from './directory';

const entry = {
	name: 'été #1.txt',
	size: 12,
	isDir: false,
	modTime: '2026-10-23T00:00:00Z',
	permissions: '-rw-r--r--',
	mimeType: 'text/plain'
};
test('expands listing paths without losing metadata or encoding names', () => {
	assert.deepEqual(parseDirectory({ path: 'media/My Photos', items: [entry] }), {
		path: 'media/My Photos',
		items: [{ ...entry, path: 'media/My Photos/été #1.txt' }]
	});
	assert.deepEqual(parseDirectory({ path: 'media', items: [] }), { path: 'media', items: [] });
});
test('rejects malformed metadata and child names at the listing boundary', () => {
	for (const name of ['..', '.', 'nested/file', 'bad\0name']) {
		assert.throws(() => parseDirectory({ path: 'media', items: [{ ...entry, name }] }));
	}
	for (const change of [{ size: '12' }, { isDir: 1 }, { permissions: null }, { name: '' }]) {
		assert.throws(() => parseDirectory({ path: 'media', items: [{ ...entry, ...change }] }));
	}
});
