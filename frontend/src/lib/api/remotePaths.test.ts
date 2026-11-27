import assert from 'node:assert/strict';
import { after, before, describe, test } from 'node:test';
import {
	parseRemotePath,
	qualifyRemotePath,
	pathEndpoint,
	invalidateRemoteServers
} from './remotePaths';
import { getFileInfo } from './files';
import { createShare, listShares } from './shareManagement';
import { uploadChunkSize } from '../utils/uploadConfig';
import { getUploadStatus, resumeUpload, uploadFile } from '../utils/upload';

const ID = '12345678-1234-4234-8234-123456789abc';
const ref = (path: string) => qualifyRemotePath(ID, path);
const permissions = {
	view: true,
	download: true,
	upload: false,
	delete: false,
	manage: false,
	canReplace: false
};
const file = {
	name: 'a.txt',
	path: 'media/a.txt',
	size: 3,
	isDir: false,
	modTime: '2026-01-01',
	permissions: 'rw'
};
const share = {
	id: 'share-1',
	token: 'public-token',
	url: '/s/public-token',
	fileName: 'a.txt',
	path: 'media/a.txt',
	isFolder: false,
	permissions,
	maxUploadBytes: 100,
	createdAt: '2026-01-01'
};
const requests: { path: string; method: string; body: unknown; query: URLSearchParams }[] = [];
let malformed = false;
let completeUpload = false;
let origin = '';

class UploadXHR {
	status = 0;
	responseText = '';
	upload = { onprogress: null };
	onload: (() => void) | null = null;
	onerror: (() => void) | null = null;
	onabort: (() => void) | null = null;
	url = '';
	headers: Record<string, string> = {};
	open(_method: string, url: string) {
		this.url = url;
	}
	setRequestHeader(key: string, value: string) {
		this.headers[key] = value;
	}
	abort() {
		this.onabort?.();
	}
	send(body: Blob) {
		void fetch(new URL(this.url, origin), { method: 'POST', body, headers: this.headers })
			.then(async (response) => {
				this.status = response.status;
				this.responseText = await response.text();
				this.onload?.();
			})
			.catch(() => this.onerror?.());
	}
}

describe('qualified references', () => {
	test('preserves local paths and encodes remote segments exactly once', () => {
		assert.deepEqual(parseRemotePath('media/a.txt'), { kind: 'local', path: 'media/a.txt' });
		assert.deepEqual(parseRemotePath(ref('media/a #%.txt')), {
			kind: 'remote',
			serverId: ID,
			path: 'media/a #%.txt'
		});
		assert.equal(
			pathEndpoint(ref('media/a #%.txt'), '/files'),
			`/desktop/servers/${ID}/api/v1/files/media/a%20%23%25.txt`
		);
	});
	test('deployment roots navigate but cannot address file endpoints', () => {
		assert.deepEqual(parseRemotePath(`@remote/${ID}`), { kind: 'remote', serverId: ID, path: '' });
		assert.throws(() => pathEndpoint(`@remote/${ID}`, '/files'), /Choose a remote drive/);
	});
	test('rejects missing mounts, invalid IDs, absolute paths, traversal and control characters', () => {
		for (const path of [
			'@remote',
			'@remote/no-id/media',
			`@remote/${ID}/`,
			`@remote/${ID}//etc`,
			ref('media') + '/../etc',
			`@remote/${ID}/media/./a`,
			`@remote/${ID}/media//a`,
			`@remote/${ID}/media\\a`,
			`@remote/${ID}/media/\u0000`
		]) {
			assert.throws(() => parseRemotePath(path), /Invalid remote/);
		}
	});
});

describe('live HTTP boundaries', () => {
	const oldWindow = globalThis.window;
	const oldDocument = globalThis.document;
	const oldStorage = globalThis.localStorage;
	const oldXHR = globalThis.XMLHttpRequest;
	const oldFetch = globalThis.fetch;
	let server: ReturnType<typeof Bun.serve>;
	before(() => {
		server = Bun.serve({
			port: 0,
			async fetch(request) {
				const url = new URL(request.url);
				const path = url.pathname;
				const body: unknown =
					request.method === 'GET' || request.method === 'DELETE'
						? undefined
						: request.headers.get('content-type')?.includes('application/json')
							? await request.json()
							: await request.text();
				requests.push({ path, method: request.method, body, query: url.searchParams });
				const endpoint = path.slice('/api/v1'.length);
				if (endpoint === '/system/upload') return Response.json({ chunkSizeBytes: 16 });
				if (endpoint.startsWith('/files/'))
					return Response.json(malformed ? { ...file, size: 'wrong' } : file);
				if (endpoint === '/shares')
					return Response.json(request.method === 'POST' ? share : { shares: [share] });
				if (endpoint === '/stream/upload/status/')
					return Response.json({
						uploadId: 'upload-1',
						path: 'media/a.txt',
						totalChunks: 1,
						receivedChunks: completeUpload ? 1 : 0,
						missingChunks: completeUpload ? [] : [0],
						complete: completeUpload,
						createdAt: '2026-01-01',
						lastActivity: '2026-01-01'
					});
				if (endpoint.startsWith('/stream/upload/'))
					return Response.json({
						uploadId: 'upload-1',
						chunkIndex: 0,
						receivedChunks: 1,
						totalChunks: 1,
						complete: true,
						path: 'media/a.txt'
					});
				return Response.json({ error: 'not found', code: 'NOT_FOUND' }, { status: 404 });
			}
		});
		origin = server.url.origin;
		Object.assign(globalThis, {
			document: { querySelector: () => null },
			window: Object.assign(new EventTarget(), { location: { origin } }),
			localStorage: { getItem: () => null },
			XMLHttpRequest: UploadXHR,
			fetch: (input: string | URL | Request, init?: RequestInit) =>
				oldFetch(typeof input === 'string' ? new URL(input, origin) : input, init)
		});
	});
	after(() => {
		server.stop(true);
		invalidateRemoteServers();
		Object.assign(globalThis, {
			document: oldDocument,
			window: oldWindow,
			localStorage: oldStorage,
			XMLHttpRequest: oldXHR,
			fetch: oldFetch
		});
	});

	test('upload configuration deduplicates concurrent loads and refreshes afterward', async () => {
		const before = requests.length;
		assert.deepEqual(
			await Promise.all([uploadChunkSize('media/a'), uploadChunkSize('media/b')]),
			[16, 16]
		);
		assert.equal(
			requests.slice(before).filter((request) => request.path.endsWith('/system/upload')).length,
			1
		);
		await uploadChunkSize('media/a');
		assert.equal(
			requests.slice(before).filter((request) => request.path.endsWith('/system/upload')).length,
			2
		);
	});

	test('rejects malformed file metadata at the network boundary', async () => {
		malformed = true;
		try {
			await assert.rejects(getFileInfo('media/a.txt'), /Invalid file size/);
		} finally {
			malformed = false;
		}
	});

	test('defaults missing deleteOnExpiry to false and forwards it only when chosen', async () => {
		const shares = await listShares();
		assert.equal(shares.shares[0].deleteOnExpiry, false);
		await createShare('media/a.txt', { expiresInSeconds: 3600, deleteOnExpiry: true });
		const creation = requests.findLast(
			(request) => request.path === '/api/v1/shares' && request.method === 'POST'
		);
		assert.deepEqual(creation?.body, {
			path: 'media/a.txt',
			expiresInSeconds: 3600,
			deleteOnExpiry: true
		});
	});

	test('uploads an empty file as one checksum-verified chunk and resumes pending uploads', async () => {
		const before = requests.length;
		const empty = await uploadFile(new File([], 'empty.txt'), 'media/empty.txt', { chunkSize: 8 });
		assert.equal(empty.success, true);
		assert.equal(
			requests.slice(before).filter((request) => request.path.includes('/stream/upload/')).length,
			1
		);

		const upload = new File(['abc'], 'a.txt');
		assert.equal((await getUploadStatus('upload-1', 'media/a.txt'))?.path, 'media/a.txt');
		const resumed = await resumeUpload(upload, 'media/a.txt', 'upload-1', { chunkSize: 10 });
		assert.equal(resumed.path, 'media/a.txt');
		completeUpload = true;
		const complete = await resumeUpload(upload, 'media/a.txt', 'upload-1');
		assert.equal(complete.path, 'media/a.txt');
	});
});
