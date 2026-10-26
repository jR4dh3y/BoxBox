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

	test('native locations include video, audio, PDF and additional raster images', () => {
		const original = Object.getOwnPropertyDescriptor(globalThis, 'document');
		Object.defineProperty(globalThis, 'document', {
			configurable: true,
			value: { querySelector: () => ({}) }
		});
		try {
			for (const extension of [
				'mp4',
				'mkv',
				'mov',
				'webm',
				'mp3',
				'flac',
				'wav',
				'pdf',
				'bmp',
				'tiff',
				'ico',
				'qoi'
			]) {
				assert.equal(
					supportsThumbnailFile({ path: `root/media/file.${extension}`, isDir: false }),
					true,
					extension
				);
			}
			assert.equal(
				supportsThumbnailFile({ path: 'root/movie', mimeType: 'video/mp4', isDir: false }),
				false
			);
			assert.equal(supportsThumbnailFile({ path: 'root/folder.mp4', isDir: true }), false);
			assert.equal(
				supportsThumbnailFile({ path: 'root/source.ts', mimeType: 'video/mp2t', isDir: false }),
				false
			);
			assert.equal(
				supportsThumbnailFile({ path: 'root/vector.svg', mimeType: 'image/svg+xml', isDir: false }),
				false
			);
		} finally {
			if (original) Object.defineProperty(globalThis, 'document', original);
			else Reflect.deleteProperty(globalThis, 'document');
		}
	});

	test('web locations retain their server-supported image formats', () => {
		for (const extension of ['mp4', 'mp3', 'pdf', 'bmp']) {
			assert.equal(supportsThumbnailFile({ path: `media/file.${extension}`, isDir: false }), false);
		}
	});
});
