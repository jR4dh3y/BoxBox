import assert from 'node:assert/strict';
import { afterEach, beforeEach, describe, test } from 'node:test';
import { BrowseRequests } from './requests.svelte';
import type { EarlyStartSource } from '#lib/api/earlyStart.js';
import type { FileList } from '#lib/api/files.js';

const noEarly: EarlyStartSource = {
	takeSession: async () => null,
	takeDirectory: async () => null
};
const realWindow = globalThis.window;
const realFetch = globalThis.fetch;
let owner: BrowseRequests;
let requests: string[];
let names: string[];
const listing = (...items: string[]): FileList => ({
	path: 'media',
	items: items.map((name) => ({
		name,
		path: `media/${name}`,
		isDir: false,
		size: 1,
		modTime: '',
		permissions: ''
	}))
});
const settle = () => new Promise((resolve) => setTimeout(resolve, 20));

beforeEach(() => {
	requests = [];
	names = ['a.txt'];
	owner = new BrowseRequests(noEarly);
	Object.assign(globalThis, {
		window: { location: { origin: 'http://localhost' } },
		fetch: async (input: string) => {
			requests.push(input);
			return Response.json(listing(...names));
		}
	});
});
afterEach(() => {
	owner.reset();
	Object.assign(globalThis, { window: realWindow, fetch: realFetch });
});

describe('source-qualified directory ownership', () => {
	test('hover prefetch deduplicates, opening shows a snapshot only during server confirmation', async () => {
		await owner.prefetch('media');
		await owner.prefetch('media');
		assert.equal(requests.length, 1);
		names.push('external.txt');
		const folder = owner.folder('media');
		const before = folder.confirmed;
		const release = owner.observe('media');
		assert.deepEqual(
			folder.value?.items.map((item) => item.name),
			['a.txt']
		);
		assert.equal(folder.pending, true);
		await folder.load();
		assert.deepEqual(
			folder.value?.items.map((item) => item.name),
			names
		);
		assert.ok(folder.confirmed > before);
		assert.equal(requests.length, 2);
		release();
		assert.equal(folder.value, undefined, 'inactive data is not shown as confirmed');
	});

	test('one request carries 10000 entries without paging/sort/hidden parameters', async () => {
		names = Array.from({ length: 10000 }, (_, index) => `file-${index}.txt`);
		await owner.prefetch('media');
		assert.equal(owner.folder('media').value?.items.length, 10000);
		assert.equal(requests.length, 1);
		assert.equal(new URL(requests[0]).search, '');
	});

	test('two pane leases and prefetch share a request; final release alone aborts it', async () => {
		let signal: AbortSignal | undefined;
		let finish = () => {};
		Object.assign(globalThis, {
			fetch: async (_input: string, init: RequestInit) => {
				signal = init.signal ?? undefined;
				await new Promise<void>((resolve) => {
					finish = resolve;
				});
				return Response.json(listing());
			}
		});
		const releaseA = owner.observe('media');
		const releaseB = owner.observe('media');
		const prefetch = owner.prefetch('media');
		await settle();
		releaseA();
		assert.equal(signal?.aborted, false);
		releaseB();
		assert.equal(signal?.aborted, true);
		finish();
		await prefetch;
		assert.equal(
			owner.folder('media').value,
			undefined,
			'late abort-ignoring transport cannot publish'
		);
	});

	test('invalidation during an in-flight listing queues exactly one follow-up and awaits it', async () => {
		let finish = () => {};
		let calls = 0;
		Object.assign(globalThis, {
			fetch: async () => {
				calls++;
				if (calls === 1)
					await new Promise<void>((resolve) => {
						finish = resolve;
					});
				return Response.json(listing(calls === 1 ? 'old.txt' : 'new.txt'));
			}
		});
		const release = owner.observe('media');
		await settle();
		let completed = false;
		const changed = Promise.all([owner.changed('media'), owner.changed('media')]).then(() => {
			completed = true;
		});
		assert.equal(completed, false);
		finish();
		await changed;
		assert.equal(calls, 2);
		assert.deepEqual(
			owner.folder('media').value?.items.map((item) => item.name),
			['new.txt']
		);
		release();
	});

	test('retry confirms once after a transient failure and failed confirmation cannot establish deletion', async () => {
		const release = owner.observe('media');
		await owner.folder('media').load();
		const revision = owner.folder('media').confirmed;
		let calls = 0;
		Object.assign(globalThis, {
			fetch: async () => {
				calls++;
				if (calls === 1) throw new Error('transient');
				return Response.json(listing('new.txt'));
			}
		});
		const started = Date.now();
		await owner.changed('media');
		assert.equal(calls, 2);
		assert.ok(Date.now() - started >= 990);
		assert.ok(owner.folder('media').confirmed > revision);
		const success = owner.folder('media').confirmed;
		Object.assign(globalThis, {
			fetch: async () => {
				throw new Error('offline');
			}
		});
		await owner.changed('media');
		assert.equal(owner.folder('media').confirmed, success);
		assert.equal(owner.folder('media').error?.message, 'offline');
		assert.equal(
			owner.folder('media').value?.items[0].name,
			'new.txt',
			'failed confirmation retains count and cannot establish deletion; UI renders its error'
		);
		release();
	});

	test('old-session response and offline retry cannot restore snapshots after reset', async () => {
		let finish = () => {};
		Object.assign(globalThis, {
			fetch: async () => {
				await new Promise<void>((resolve) => {
					finish = resolve;
				});
				return Response.json(listing('private.txt'));
			}
		});
		const old = owner.folder('media');
		const release = owner.observe('media');
		const loading = old.load();
		await settle();
		owner.reset();
		owner.startSession();
		Object.assign(globalThis, { fetch: async () => Response.json(listing('new-session.txt')) });
		await owner.prefetch('media');
		finish();
		await loading;
		assert.equal(old.value, undefined);
		assert.deepEqual(
			owner.folder('media').value?.items.map((item) => item.name),
			['new-session.txt']
		);
		release();
	});

	test('source identity separates equal local and remote paths', async () => {
		const remote = '@remote/12345678-1234-1234-1234-123456789012/media';
		const a = owner.observe('media');
		const b = owner.observe(remote);
		await Promise.all([owner.folder('media').load(), owner.folder(remote).load()]);
		assert.equal(requests.length, 2);
		assert.equal(owner.folder(remote).value?.path, remote);
		assert.notEqual(owner.folder('media'), owner.folder(remote));
		a();
		b();
	});

	test('a superseded search cannot publish into the next search or survive release', async () => {
		let finish = () => {};
		Object.assign(globalThis, {
			fetch: async (input: string) => {
				if (new URL(input).searchParams.get('q') === 'old')
					await new Promise<void>((resolve) => {
						finish = resolve;
					});
				return Response.json({
					path: 'media',
					query: new URL(input).searchParams.get('q'),
					count: 1,
					results: listing('match.txt').items
				});
			}
		});
		const old = owner.matches('media', 'old');
		const release = owner.observeSearch('media', 'old');
		const loading = old.load();
		await settle();
		release();
		const next = owner.matches('media', 'new');
		const stop = owner.observeSearch('media', 'new');
		await next.load();
		finish();
		await loading;
		assert.equal(old.value, undefined);
		assert.equal(next.value?.results[0].name, 'match.txt');
		stop();
	});

	test('inactive folder cache is bounded while a live pane remains retained', async () => {
		const release = owner.observe('media');
		await owner.folder('media').load();
		const active = owner.folder('media');
		const first = owner.folder('media/first');
		await owner.prefetch('media/first');
		for (let i = 0; i < 40; i++) await owner.prefetch(`media/${i}`);
		assert.equal(owner.folder('media'), active);
		assert.notEqual(owner.folder('media/first'), first);
		release();
	});
});

describe('shell listing handoff', () => {
	test('uses a matching early response without another listing', async () => {
		owner = new BrowseRequests({
			...noEarly,
			takeDirectory: async (url) => {
				assert.match(url, /\/files\/list\/media$/);
				return listing('early.txt');
			}
		});
		await owner.prefetch('media');
		assert.equal(owner.folder('media').value?.items[0].name, 'early.txt');
		assert.equal(requests.length, 0);
	});
	test('falls back to network when the shell has nothing usable', async () => {
		await owner.prefetch('media');
		assert.equal(requests.length, 1);
	});
});

describe('shared catalog lifecycle', () => {
	test('roots and space deduplicate across panes; closing one does not abort the other', async () => {
		const signals: AbortSignal[] = [];
		Object.assign(globalThis, {
			fetch: async (input: string, init: RequestInit) => {
				requests.push(input);
				if (init.signal) signals.push(init.signal);
				return Response.json(input.endsWith('/stats') ? { drives: [] } : { roots: [] });
			}
		});
		const a = owner.observeCatalog(true);
		const b = owner.observeCatalog(true);
		await Promise.all([owner.catalog().roots.load(), owner.catalog().stats.load()]);
		assert.equal(requests.length, 2);
		a();
		assert.equal(
			signals.some((signal) => signal.aborted),
			false
		);
		await owner.changed();
		assert.equal(requests.length, 4);
		b();
	});
});

describe('workspace session and job boundaries', () => {
	test('job completion confirms live directory and space through the same domain hook as layout', async () => {
		const { jobsStore } = await import('#lib/stores/jobs.svelte.js');
		jobsStore.reset();
		const server = Bun.serve({
			port: 0,
			fetch(request) {
				const path = new URL(request.url).pathname;
				requests.push(path);
				if (path.endsWith('/stats')) return Response.json({ drives: [] });
				if (path.endsWith('/files')) return Response.json({ roots: [] });
				return Response.json(listing(...names));
			}
		});
		Object.assign(globalThis, {
			window: { location: { origin: server.url.origin } },
			fetch: realFetch
		});
		const release = owner.observe('media');
		const catalog = owner.observeCatalog(true);
		try {
			await Promise.all([
				owner.folder('media').load(),
				owner.catalog().roots.load(),
				owner.catalog().stats.load()
			]);
			let confirmation = Promise.resolve();
			jobsStore.onFinished = () => {
				confirmation = owner.changed();
			};
			jobsStore.upsertJob({
				id: 'domain-job',
				type: 'delete',
				state: 'running',
				progress: 0,
				sourcePath: 'media/a.txt',
				createdAt: ''
			});
			names = ['external.txt'];
			jobsStore.updateFromWebSocket({ jobId: 'domain-job', state: 'running', progress: 50 });
			assert.equal(requests.length, 3);
			jobsStore.updateFromWebSocket({ jobId: 'domain-job', state: 'completed', progress: 100 });
			await confirmation;
			assert.equal(requests.length, 6);
			assert.equal(owner.folder('media').value?.items[0].name, 'external.txt');
		} finally {
			jobsStore.onFinished = undefined;
			jobsStore.reset();
			release();
			catalog();
			server.stop(true);
		}
	});

	test('offline pending work is aborted by session reset and reconnect cannot resurrect it', async () => {
		const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
		const events = Object.assign(new EventTarget(), { location: { origin: 'http://localhost' } });
		Object.defineProperty(globalThis, 'navigator', {
			configurable: true,
			value: { onLine: false }
		});
		Object.assign(globalThis, { window: events });
		const release = owner.observe('media');
		const old = owner.folder('media');
		const loading = old.load();
		try {
			await settle();
			assert.equal(requests.length, 0);
			owner.reset();
			await loading;
			Object.defineProperty(globalThis, 'navigator', {
				configurable: true,
				value: { onLine: true }
			});
			events.dispatchEvent(new Event('online'));
			await settle();
			assert.equal(requests.length, 0);
			assert.equal(old.value, undefined);
			owner.startSession();
			await owner.prefetch('media');
			assert.equal(requests.length, 1);
		} finally {
			release();
			if (descriptor) Object.defineProperty(globalThis, 'navigator', descriptor);
			else Reflect.deleteProperty(globalThis, 'navigator');
		}
	});
});

test('offline native desktop confirms local folders, search, catalog and prefetch over HTTP', async () => {
	const navigator = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
	const document = Object.getOwnPropertyDescriptor(globalThis, 'document');
	const events = Object.assign(new EventTarget(), { location: { origin: '' } });
	const server = Bun.serve({
		port: 0,
		fetch(request) {
			const url = new URL(request.url);
			requests.push(url.pathname);
			if (url.pathname.endsWith('/stats')) return Response.json({ drives: [] });
			if (url.pathname.endsWith('/files')) return Response.json({ roots: [] });
			if (url.pathname.endsWith('/search'))
				return Response.json({
					path: 'media',
					query: 'a',
					count: 1,
					results: listing('a.txt').items
				});
			return Response.json(listing('a.txt'));
		}
	});
	events.location.origin = server.url.origin;
	Object.assign(globalThis, { window: events, fetch: realFetch });
	Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { onLine: false } });
	Object.defineProperty(globalThis, 'document', {
		configurable: true,
		value: { querySelector: () => ({}) }
	});
	const releases = [
		owner.observe('media'),
		owner.observeSearch('media', 'a'),
		owner.observeCatalog(true)
	];
	const prefetch = owner.prefetch('media/child');
	try {
		await new Promise((resolve) => setTimeout(resolve, 150));
		assert.equal(requests.length, 5);
		assert.equal(owner.folder('media').value?.items[0].name, 'a.txt');
		assert.equal(owner.matches('media', 'a').value?.results[0].name, 'a.txt');
		assert.deepEqual(owner.catalog().roots.value?.roots, []);
		assert.deepEqual(owner.catalog().stats.value?.drives, []);
		assert.equal(owner.folder('media/child').value?.items[0].name, 'a.txt');
		const remote = '@remote/12345678-1234-1234-1234-123456789012/media';
		releases.push(owner.observe(remote), owner.observeSearch(remote, 'a'));
		await settle();
		assert.equal(requests.length, 5, 'qualified remote work still waits offline');
		owner.reset();
		events.dispatchEvent(new Event('online'));
		await settle();
		assert.equal(requests.length, 5, 'reset cancels offline remote waits');
	} finally {
		owner.reset();
		for (const release of releases) release();
		await prefetch;
		if (navigator) Object.defineProperty(globalThis, 'navigator', navigator);
		else Reflect.deleteProperty(globalThis, 'navigator');
		if (document) Object.defineProperty(globalThis, 'document', document);
		else Reflect.deleteProperty(globalThis, 'document');
		server.stop(true);
	}
});
