import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { getThumbnailSourceKey, supportsThumbnailFile } from './thumbnail';

describe('thumbnail helpers', () => {
	test('source key changes with path, modtime, and size', () => {
		const file = { path: 'media/photo.jpg', modTime: '2026-10-22T00:00:00Z', size: 1200 };
		const key = getThumbnailSourceKey(file);
		assert.equal(getThumbnailSourceKey(file), key);
		assert.notEqual(getThumbnailSourceKey({ ...file, path: 'media/other.jpg' }), key);
		assert.notEqual(getThumbnailSourceKey({ ...file, modTime: '2026-10-22T00:00:01Z' }), key);
		assert.notEqual(getThumbnailSourceKey({ ...file, size: 1201 }), key);
	});

	test('only formats decoded by the thumbnail service request a rendition', () => {
		for (const path of [
			'media/a.jpg',
			'media/a.jpeg',
			'media/a.PNG',
			'media/a.gif',
			'media/a.webp'
		]) {
			assert.equal(supportsThumbnailFile({ path, isDir: false }), true, path);
		}
		assert.equal(supportsThumbnailFile({ path: 'media/a.svg', isDir: false }), false);
		assert.equal(
			supportsThumbnailFile({ path: 'media/no-extension', mimeType: 'image/webp', isDir: false }),
			true
		);
		assert.equal(supportsThumbnailFile({ path: 'media/folder.jpg', isDir: true }), false);
	});
});
