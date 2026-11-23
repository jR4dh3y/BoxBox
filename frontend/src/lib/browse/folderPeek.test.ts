import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import type { FileInfo } from '#lib/api/files.js';
import { FolderPeeks } from './folderPeek.svelte.js';

const folder = (modTime = '2026-01-01T00:00:00Z'): FileInfo => ({
	name: 'album',
	path: 'media/album',
	size: 0,
	isDir: true,
	modTime,
	permissions: 'drwxr-xr-x'
});
const cover: FileInfo = {
	...folder(),
	name: 'cover.jpg',
	path: 'media/album/cover.jpg',
	isDir: false
};

describe('folder peeks', () => {
	test('reads a folder once, and again when it changes', async () => {
		const reads: string[] = [];
		const peeks = new FolderPeeks(async (path) => {
			reads.push(path);
			return [cover];
		});
		peeks.request(folder(), false);
		peeks.request(folder(), false);
		await Promise.resolve();
		await Promise.resolve();
		assert.deepEqual(peeks.get(folder(), false), [cover]);
		assert.equal(reads.length, 1);
		peeks.request(folder('2026-02-01T00:00:00Z'), false);
		assert.equal(reads.length, 2);
	});

	test('a session reset drops cached entries and rejects an earlier reply for the same folder', async () => {
		const previous = Promise.withResolvers<FileInfo[]>();
		const next = Promise.withResolvers<FileInfo[]>();
		const replies = [Promise.resolve([cover]), previous.promise, next.promise];
		const peeks = new FolderPeeks(() => replies.shift() ?? Promise.resolve([]));
		const changed = folder('2026-02-01T00:00:00Z');
		peeks.request(folder(), false);
		await Promise.resolve();
		peeks.request(changed, false);
		peeks.clear();
		assert.equal(peeks.get(folder(), false), undefined);
		peeks.request(changed, false);
		previous.resolve([cover]);
		await previous.promise;
		assert.equal(peeks.get(changed, false), undefined);
		next.resolve([]);
		await next.promise;
		assert.deepEqual(peeks.get(changed, false), []);
	});

	test('evicts a completed preview without losing other replies', async () => {
		const peeks = new FolderPeeks(async () => [cover]);
		const numberedFolder = (index: number): FileInfo => ({
			...folder(),
			path: `media/album-${index}`
		});
		for (let index = 0; index < 1024; index++) peeks.request(numberedFolder(index), false);
		await Promise.resolve();
		peeks.request(numberedFolder(1024), false);
		peeks.request(numberedFolder(1025), false);
		await Promise.resolve();
		assert.equal(peeks.get(numberedFolder(0), false), undefined);
		assert.deepEqual(peeks.get(numberedFolder(1024), false), [cover]);
		assert.deepEqual(peeks.get(numberedFolder(1025), false), [cover]);
	});

	test('an unreadable folder is drawn plain, and files are never read', async () => {
		let reads = 0;
		const peeks = new FolderPeeks(async () => {
			reads++;
			throw new Error('404');
		});
		peeks.request(cover, false);
		peeks.request(folder(), false);
		await Promise.resolve();
		await Promise.resolve();
		assert.equal(reads, 1);
		assert.deepEqual(peeks.get(folder(), false), []);
	});

	test('a failed read is tried again once a while has passed', async () => {
		let now = 0;
		let fail = true;
		const peeks = new FolderPeeks(
			async () => {
				if (fail) throw new Error('offline');
				return [cover];
			},
			() => now
		);
		peeks.request(folder(), false);
		await Promise.resolve();
		await Promise.resolve();
		assert.deepEqual(peeks.get(folder(), false), []);
		fail = false;
		peeks.request(folder(), false);
		await Promise.resolve();
		await Promise.resolve();
		assert.deepEqual(peeks.get(folder(), false), [], 'not straight away');
		now = 60_000;
		peeks.request(folder(), false);
		await Promise.resolve();
		await Promise.resolve();
		assert.deepEqual(peeks.get(folder(), false), [cover]);
	});
});
