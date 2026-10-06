import assert from 'node:assert/strict';
import { afterEach, beforeEach, test } from 'node:test';
import { uploadShareEntries, uploadShareFile } from './uploads';

class UploadRequest {
	static requests: UploadRequest[] = [];
	static onSend: (request: UploadRequest) => void = (request) => request.onload?.();
	status = 201;
	url = '';
	aborted = false;
	upload: { onprogress: ((event: ProgressEvent) => void) | null } = { onprogress: null };
	onload: (() => void) | null = null;
	onerror: (() => void) | null = null;
	onabort: (() => void) | null = null;
	open(_method: string, url: string) {
		this.url = url;
		UploadRequest.requests.push(this);
	}
	send() {
		UploadRequest.onSend(this);
	}
	abort() {
		this.aborted = true;
		this.onabort?.();
	}
}

const originalRequest = globalThis.XMLHttpRequest;
beforeEach(() => {
	UploadRequest.requests = [];
	UploadRequest.onSend = (request) => request.onload?.();
	Object.assign(globalThis, { XMLHttpRequest: UploadRequest });
});
afterEach(() => Object.assign(globalThis, { XMLHttpRequest: originalRequest }));

const progress = () => {};

test('an editor save leaves a concurrent batch upload percentage unchanged', async () => {
	UploadRequest.onSend = () => {};
	let batchProgress = 0;
	const batch = uploadShareEntries({
		token: 'share',
		path: '',
		entries: [{ file: new File(['batch'], 'batch.txt'), relativePath: 'batch.txt' }],
		signal: new AbortController().signal,
		onFile: progress,
		onProgress: (percent) => (batchProgress = percent)
	});
	const batchRequest = UploadRequest.requests[0];
	batchRequest.upload.onprogress?.(
		Object.assign(new Event('progress'), { lengthComputable: true, loaded: 40, total: 100 })
	);
	assert.equal(batchProgress, 40);

	const save = uploadShareFile({ token: 'share', path: 'edit.txt', file: new Blob(['saved']) });
	const saveRequest = UploadRequest.requests[1];
	assert.equal(batchProgress, 40);
	saveRequest.upload.onprogress?.(
		Object.assign(new Event('progress'), { lengthComputable: true, loaded: 80, total: 100 })
	);
	assert.equal(batchProgress, 40);
	saveRequest.onload?.();
	await save;
	batchRequest.onload?.();
	await batch;
});

test('a batch keeps its original share and folder across asynchronous file requests', async () => {
	const destination = {
		token: 'original-share',
		path: 'original-folder',
		entries: ['a.txt', 'b.txt'].map((name) => ({
			file: new File(['content'], name),
			relativePath: name
		})),
		signal: new AbortController().signal,
		onFile: progress,
		onProgress: progress
	};
	UploadRequest.onSend = (request) => {
		destination.token = 'another-share';
		destination.path = 'another-folder';
		request.onload?.();
	};
	await uploadShareEntries(destination);
	assert.deepEqual(
		UploadRequest.requests.map((request) => request.url),
		[
			'/api/v1/share/original-share/upload?path=original-folder%2Fa.txt',
			'/api/v1/share/original-share/upload?path=original-folder%2Fb.txt'
		]
	);
});

test('cancelling a batch aborts network I/O and stops the next file', async () => {
	const controller = new AbortController();
	UploadRequest.onSend = () => controller.abort();
	await assert.rejects(
		uploadShareEntries({
			token: 'share',
			path: '',
			entries: ['a.txt', 'b.txt'].map((name) => ({
				file: new File(['content'], name),
				relativePath: name
			})),
			signal: controller.signal,
			onFile: progress,
			onProgress: progress
		}),
		{ name: 'AbortError' }
	);
	assert.equal(UploadRequest.requests.length, 1);
	assert.equal(UploadRequest.requests[0].aborted, true);
});

test('a forbidden upload reports a permission denial without claiming a replacement', async () => {
	UploadRequest.onSend = (request) => {
		request.status = 403;
		request.onload?.();
	};
	await assert.rejects(
		uploadShareFile({
			token: 'share',
			path: 'new.txt',
			file: new Blob(['content']),
			onProgress: progress
		}),
		{ message: 'This link does not allow this upload.' }
	);
});
