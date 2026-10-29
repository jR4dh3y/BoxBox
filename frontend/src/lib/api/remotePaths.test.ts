import assert from 'node:assert/strict';
import { after, before, describe, test } from 'node:test';
import {
	parseRemotePath,
	qualifyRemotePath,
	pathEndpoint,
	listRemoteServers,
	invalidateRemoteServers
} from './remotePaths';
import {
	createDirectory,
	createFile,
	deleteFile,
	directoryUrl,
	getDirectoryArchiveUrl,
	getDownloadUrl,
	getDriveStats,
	getFileInfo,
	getPreviewUrl,
	getThumbnailUrl,
	listDirectory,
	listRoots,
	rename,
	saveFileContent,
	search
} from './files';
import { cancelJob, createCopyJob, createDeleteJob, createMoveJob, getJob, listJobs } from './jobs';
import { createShare, listShares, revokeShare } from './shareManagement';
import { uploadChunkSize } from '../utils/uploadConfig';
import {
	listDesktopServers,
	saveDesktopServer,
	pairDesktopServer,
	removeDesktopServer
} from './desktopServers';
import { probeDesktopServer } from '../desktop/servers.svelte';
import { getUploadStatus, resumeUpload, uploadFile } from '../utils/upload';

const ID = '12345678-1234-4234-8234-123456789abc';
const OFFLINE = '23456789-1234-4234-8234-123456789abc';
const ref = (path: string) => qualifyRemotePath(ID, path);
const proxy = `/api/v1/desktop/servers/${ID}/api/v1`;
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
const job = {
	id: 'job-1',
	type: 'copy',
	state: 'running',
	progress: 0,
	sourcePath: 'media/a.txt',
	destPath: 'media/b.txt',
	createdAt: '2026-01-01'
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
let catalogCalls = 0;
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

describe('transport facade at HTTP boundary', () => {
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
				if (path === '/api/v1/desktop/servers') {
					if (request.method === 'POST')
						return Response.json({
							id: ID,
							name: 'Home',
							url: 'https://home.example/box',
							expiresAt: '2027-01-01'
						});
					catalogCalls++;
					await new Promise((resolve) => setTimeout(resolve, 5));
					return Response.json({
						servers: [
							{ id: ID, name: 'Home', url: 'https://home.example/box', expiresAt: '2027-01-01' },
							{
								id: OFFLINE,
								name: 'Offline',
								url: 'https://offline.example',
								expiresAt: '2027-01-01'
							}
						]
					});
				}
				if (
					path === `/api/v1/desktop/servers/${ID}` ||
					path === `/api/v1/desktop/servers/${ID}/pair`
				)
					return request.method === 'DELETE'
						? new Response(null, { status: 204 })
						: Response.json({
								id: ID,
								name: 'Renamed',
								url: 'https://home.example/box',
								expiresAt: '2027-01-01'
							});
				if (path.includes(OFFLINE))
					return Response.json({ error: 'Offline', code: 'REMOTE_UNAVAILABLE' }, { status: 502 });
				const endpoint = path.startsWith(proxy)
					? path.slice(proxy.length)
					: path.slice('/api/v1'.length);
				if (endpoint === '/system/upload')
					return Response.json({ chunkSizeBytes: path.startsWith(proxy) ? 8 : 16 });
				if (endpoint === '/files')
					return Response.json({
						roots: [{ name: 'media', path: '/private/real/path', readOnly: false, kind: '' }]
					});
				if (endpoint === '/files/stats')
					return Response.json({
						drives: [
							{
								name: 'media',
								path: '/private/real/path',
								device: '/dev/private',
								mountPoint: '/secret',
								totalBytes: 100,
								fsType: 'ext4',
								freeBytes: 50,
								usedBytes: 50,
								usedPct: 50,
								readOnly: false
							}
						]
					});
				if (endpoint.startsWith('/files/list/'))
					return Response.json({
						path: 'media',
						items: [
							{
								name: file.name,
								size: file.size,
								isDir: file.isDir,
								modTime: file.modTime,
								permissions: file.permissions
							}
						]
					});
				if (endpoint.startsWith('/files/'))
					return Response.json(
						request.method === 'DELETE'
							? { message: 'deleted' }
							: malformed
								? { ...file, size: 'wrong' }
								: file
					);
				if (endpoint === '/search')
					return Response.json({ path: 'media', query: 'a', results: [file], count: 1 });
				if (endpoint === '/jobs' || endpoint === '/desktop/transfers') {
					const result =
						endpoint === '/desktop/transfers'
							? { ...job, id: 'transfer-1', sourcePath: ref('media/a.txt'), destPath: 'local' }
							: job;
					return Response.json(request.method === 'POST' ? result : { jobs: [result] });
				}
				if (endpoint.startsWith('/jobs/') || endpoint.startsWith('/desktop/transfers/'))
					return Response.json(
						request.method === 'DELETE'
							? { message: 'cancelled' }
							: { ...job, id: endpoint.split('/').at(-1) }
					);
				if (endpoint === '/shares')
					return Response.json(request.method === 'POST' ? share : { shares: [share] });
				if (endpoint.startsWith('/shares/')) return Response.json({ success: true });
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
			document: { querySelector: () => ({}) },
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

	test('pairs, renames without changing authority, re-pairs and removes through private HTTP API', async () => {
		assert.equal((await listDesktopServers())[0]?.id, ID);
		await saveDesktopServer({
			kind: 'create',
			name: 'Home',
			url: 'https://home.example/box',
			icon: 'cloud',
			code: 'ABCD-EFGH'
		});
		assert.deepEqual(requests.at(-1)?.body, {
			name: 'Home',
			url: 'https://home.example/box',
			icon: 'cloud',
			code: 'ABCD-EFGH',
			clientName: 'BoxBox Desktop'
		});
		await saveDesktopServer({ kind: 'update', id: ID, name: 'Renamed', icon: 'home' });
		assert.deepEqual(requests.at(-1)?.body, { name: 'Renamed', icon: 'home' });
		await pairDesktopServer(ID, 'JKLM-NPQR');
		assert.deepEqual(requests.at(-1)?.body, { code: 'JKLM-NPQR', clientName: 'BoxBox Desktop' });
		await removeDesktopServer(ID);
		assert.equal(requests.at(-1)?.method, 'DELETE');
	});
	test('management probes distinguish reachable roots from offline servers', async () => {
		assert.deepEqual(await probeDesktopServer(ID), [{ name: 'media', path: ref('media') }]);
		await assert.rejects(probeDesktopServer(OFFLINE), /Offline/);
	});
	test('upload configuration is scoped by destination and refreshed after concurrent loads', async () => {
		const before = requests.length;
		assert.deepEqual(
			await Promise.all([
				uploadChunkSize(ref('media/a')),
				uploadChunkSize(ref('media/b')),
				uploadChunkSize('local/a')
			]),
			[8, 8, 16]
		);
		assert.equal(
			requests.slice(before).filter((request) => request.path.endsWith('/system/upload')).length,
			2
		);
		await uploadChunkSize(ref('media/a'));
		assert.equal(
			requests.slice(before).filter((request) => request.path.endsWith('/system/upload')).length,
			3
		);
	});

	test('local catalogs do not probe remote hosts; explicit remote catalogs qualify paths', async () => {
		const before = catalogCalls;
		const [roots, stats] = await Promise.all([listRoots(), getDriveStats()]);
		assert.equal(catalogCalls, before);
		assert.equal(roots.roots.length, 1);
		assert.equal(roots.roots[0].path, '/private/real/path');
		assert.equal(stats.drives.length, 1);
		const servers = await listRemoteServers();
		const server = servers.find((server) => server.id === ID);
		assert.ok(server);
		const [remoteRoots, remoteStats] = await Promise.all([
			listRoots(server),
			getDriveStats(server)
		]);
		assert.equal(remoteRoots.roots[0].name, ref('media'));
		assert.equal(remoteRoots.roots[0].path, ref('media'));
		assert.equal(remoteRoots.roots[0].serverName, 'Home');
		assert.equal(remoteStats.drives[0].path, ref('media'));
		assert.equal(remoteStats.drives[0].fsType, 'ext4');
		assert.equal(remoteStats.drives[0].device, undefined);
		assert.equal(remoteStats.drives[0].mountPoint, undefined);
	});

	test('qualifies listing/stat/search/create results and strips only source prefixes from requests', async () => {
		assert.equal((await getFileInfo(ref('media/a.txt'))).path, ref('media/a.txt'));
		assert.equal((await getFileInfo('media/a.txt')).path, 'media/a.txt');
		const list = await listDirectory(ref('media'));
		assert.equal(list.path, ref('media'));
		assert.equal(list.items[0].path, ref('media/a.txt'));
		assert.equal(
			requests.at(-1)?.query.size,
			0,
			'whole-folder listing sends no pagination or filtering'
		);
		assert.equal(new URL(directoryUrl(ref('media'))).pathname, `${proxy}/files/list/media`);
		assert.equal((await createDirectory(ref('media'), 'folder')).path, ref('media/a.txt'));
		assert.equal((await createFile(ref('media'), 'a.txt', 'abc')).path, ref('media/a.txt'));
		assert.equal((await saveFileContent(ref('media/a.txt'), 'abc')).path, ref('media/a.txt'));
		await rename(ref('media/a.txt'), ref('media/b.txt'));
		assert.deepEqual(requests.at(-1)?.body, { newPath: 'media/b.txt' });
		const count = requests.length;
		await assert.rejects(rename(ref('media/a.txt'), 'local/b.txt'), /same source/);
		assert.equal(requests.length, count);
		await deleteFile(ref('media/a.txt'), true);
		assert.equal(requests.at(-1)?.query.get('confirm'), 'true');
		const found = await search(ref('media'), 'a');
		assert.equal(found.results[0].path, ref('media/a.txt'));
		assert.equal(requests.at(-1)?.query.get('path'), 'media');
	});

	test('rejects malformed file metadata at the network boundary', async () => {
		malformed = true;
		try {
			await assert.rejects(getFileInfo(ref('media/a.txt')), /Invalid file size/);
		} finally {
			malformed = false;
		}
	});

	test('routes every media URL to its source', () => {
		for (const [build, route] of [
			[getPreviewUrl, 'preview'],
			[getDownloadUrl, 'download'],
			[getDirectoryArchiveUrl, 'archive']
		] as const) {
			assert.equal(build(ref('media/a #.txt')), `${proxy}/stream/${route}/media/a%20%23.txt`);
			assert.equal(build('media/a.txt'), `/api/v1/stream/${route}/media/a.txt`);
		}
		assert.equal(
			getThumbnailUrl(ref('media/a.txt'), 20, 30),
			`${proxy}/stream/thumbnail/media/a.txt?width=20&height=30`
		);
	});

	test('routes jobs, destination bodies and transfer IDs without changing the active source', async () => {
		const created = await createCopyJob(ref('media/a.txt'), ref('media/b.txt'));
		assert.equal(created.id, ref('job-1'));
		assert.equal(created.destPath, ref('media/b.txt'));
		assert.deepEqual(requests.at(-1)?.body, {
			type: 'copy',
			sourcePath: 'media/a.txt',
			destPath: 'media/b.txt'
		});
		await createDeleteJob(ref('media/a.txt'));
		assert.equal(requests.at(-1)?.path, `${proxy}/jobs`);
		await createMoveJob('local/a', 'local/b');
		assert.equal(requests.at(-1)?.path, '/api/v1/jobs');
		const transfer = await createCopyJob(ref('media/a.txt'), 'local');
		assert.equal(transfer.id, 'transfer-1');
		assert.deepEqual(requests.at(-1)?.body, {
			type: 'copy',
			sourcePath: ref('media/a.txt'),
			destPath: 'local'
		});
		await getJob(created.id);
		assert.equal(requests.at(-1)?.path, `${proxy}/jobs/job-1`);
		await cancelJob(created.id);
		assert.equal(requests.at(-1)?.path, `${proxy}/jobs/job-1`);
		await getJob(transfer.id);
		assert.equal(requests.at(-1)?.path, '/api/v1/desktop/transfers/transfer-1');
		await cancelJob(transfer.id);
		assert.equal(requests.at(-1)?.path, '/api/v1/desktop/transfers/transfer-1');
		const jobs = await listJobs();
		assert.deepEqual(
			jobs.jobs.map((job) => job.id),
			['job-1', ref('job-1'), 'transfer-1']
		);
	});

	test('makes remote share URLs public upstream URLs and preserves the owning server on revoke', async () => {
		const created = await createShare(ref('media/a.txt'), {});
		assert.equal(created.id, ref('share-1'));
		assert.equal(created.url, 'https://home.example/box/s/public-token');
		const creation = requests.findLast(
			(request) => request.path === `${proxy}/shares` && request.method === 'POST'
		);
		assert.deepEqual(creation?.body, { path: 'media/a.txt', expiresInSeconds: null });
		await revokeShare(created.id);
		assert.equal(requests.at(-1)?.path, `${proxy}/shares/share-1`);
		const shares = await listShares();
		assert.equal(shares.shares.length, 2);
		assert.equal(shares.shares[1].path, ref('media/a.txt'));
		assert.equal(shares.shares[1].url, 'https://home.example/box/s/public-token');
	});

	test('uploads an empty file as one checksum-verified chunk', async () => {
		const before = requests.length;
		const result = await uploadFile(new File([], 'empty.txt'), ref('media/empty.txt'), {
			chunkSize: 8
		});
		assert.equal(result.success, true);
		assert.equal(
			requests.slice(before).filter((request) => request.path.includes('/stream/upload/')).length,
			1
		);
	});

	test('routes upload bytes, status and resumed chunks to the destination source', async () => {
		const file = new File(['abc'], 'a.txt');
		const result = await uploadFile(file, ref('media/a.txt'), {
			uploadId: 'upload-1',
			chunkSize: 10
		});
		assert.deepEqual(result, { success: true, path: ref('media/a.txt') });
		assert.equal(requests.at(-1)?.path, `${proxy}/stream/upload/media/a.txt`);
		assert.equal(requests.at(-1)?.body, 'abc');
		assert.equal((await getUploadStatus('upload-1', ref('media/a.txt')))?.path, ref('media/a.txt'));
		assert.equal(requests.at(-1)?.path, `${proxy}/stream/upload/status/`);
		const resumed = await resumeUpload(file, ref('media/a.txt'), 'upload-1', { chunkSize: 10 });
		assert.equal(resumed.path, ref('media/a.txt'));
		assert.equal(requests.at(-1)?.path, `${proxy}/stream/upload/media/a.txt`);
		completeUpload = true;
		const complete = await resumeUpload(file, ref('media/a.txt'), 'upload-1');
		assert.equal(complete.path, ref('media/a.txt'));
		assert.equal(requests.at(-1)?.path, `${proxy}/stream/upload/status/`);
	});
});
