import assert from 'node:assert/strict';
import { test } from 'node:test';
import { flushSync } from 'svelte';
import type { UploadProgress } from '#lib/utils/upload.js';
import { createUploadAnnouncements } from './uploadAnnouncements.svelte';

test('announces batched successes once, without progress, failure or cancellation announcements', () => {
	let uploads = $state.raw<Pick<UploadProgress, 'uploadId' | 'fileName' | 'status'>[]>([]);
	let announcement!: ReturnType<typeof createUploadAnnouncements>;
	const stop = $effect.root(() => {
		announcement = createUploadAnnouncements(() => uploads);
	});
	try {
		flushSync();
		assert.equal(announcement.message, '');
		uploads = [
			{ uploadId: 'one', fileName: 'one.txt', status: 'complete' },
			{ uploadId: 'two', fileName: 'two.txt', status: 'complete' },
			{ uploadId: 'three', fileName: 'three.txt', status: 'uploading' },
			{ uploadId: 'error', fileName: 'error.txt', status: 'error' },
			{ uploadId: 'cancelled', fileName: 'cancelled.txt', status: 'cancelled' }
		];
		flushSync();
		assert.equal(announcement.message, 'one.txt, two.txt uploaded. 2 complete.');
		uploads = [...uploads];
		flushSync();
		assert.equal(announcement.message, 'one.txt, two.txt uploaded. 2 complete.');
		uploads = [{ uploadId: 'three', fileName: 'three.txt', status: 'complete' }];
		flushSync();
		assert.equal(announcement.message, 'three.txt uploaded. 3 complete.');
		uploads = [];
		flushSync();
		assert.equal(announcement.message, 'three.txt uploaded. 3 complete.');
	} finally {
		stop();
	}
});
