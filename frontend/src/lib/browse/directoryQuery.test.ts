import assert from 'node:assert/strict';
import { afterEach, beforeEach, describe, test } from 'node:test';
import { QueryClient, QueryObserver } from '@tanstack/svelte-query';
import { CONFIG } from '#lib/config.js';
import type { EarlyStartSource } from '#lib/api/earlyStart.js';
import type { FileInfo, FileList } from '#lib/api/files.js';
import { directoryQueryOptions } from './directoryQuery';

const realFetch = globalThis.fetch;
const realWindow = globalThis.window;
let listRequests = 0;
let requestedUrls: string[] = [];
let names: string[] = [];

function entry(name: string): FileInfo {
	return {
		name,
		path: `media/${name}`,
		isDir: false,
		size: 1,
		modTime: '',
		permissions: ''
	};
}

function listing(...entries: string[]): FileList {
	return { path: 'media', items: entries.map(entry) };
}

beforeEach(() => {
	listRequests = 0;
	requestedUrls = [];
	names = ['a.txt'];
	Object.assign(globalThis, {
		// The API client builds its URLs from the page origin.
		window: { location: { origin: 'http://localhost' } },
		fetch: async (input: string) => {
			listRequests++;
			requestedUrls.push(input);
			return Response.json(listing(...names));
		}
	});
});

afterEach(() => Object.assign(globalThis, { fetch: realFetch, window: realWindow }));

const settle = () => new Promise((resolve) => setTimeout(resolve, 20));
const noEarlyStart: EarlyStartSource = {
	takeSession: async () => null,
	takeDirectory: async () => null
};

describe('open folder freshness', () => {
	test('a hover prefetch fills the cache once, however often the pointer returns', async () => {
		const queryClient = new QueryClient();
		const prefetch = () =>
			queryClient.prefetchQuery({
				...directoryQueryOptions('media', noEarlyStart),
				staleTime: CONFIG.query.prefetchStaleTimeMs
			});
		await prefetch();
		await prefetch();
		assert.equal(listRequests, 1);
	});

	test('opening a prefetched folder shows it at once, then confirms it with the server', async () => {
		const queryClient = new QueryClient();
		await queryClient.prefetchQuery({
			...directoryQueryOptions('media', noEarlyStart),
			staleTime: CONFIG.query.prefetchStaleTimeMs
		});
		names = ['a.txt', 'added-outside.txt']; // a change made after the prefetch

		const observer = new QueryObserver(queryClient, directoryQueryOptions('media', noEarlyStart));
		const unsubscribe = observer.subscribe(() => {});
		const atOnce = observer.getCurrentResult().data?.items.map((item) => item.name);
		assert.deepEqual(atOnce, ['a.txt'], 'the cached listing must not wait for the network');

		await settle();
		assert.equal(listRequests, 2, 'the open folder must always ask the server');
		const confirmed = observer.getCurrentResult().data?.items.map((item) => item.name);
		assert.deepEqual(confirmed, ['a.txt', 'added-outside.txt']);
		unsubscribe();
	});

	test('one request carries the whole folder, with no paging or sort parameters', async () => {
		names = Array.from({ length: 10_000 }, (_, index) => `file-${index}.txt`);
		const data = await new QueryClient().fetchQuery(directoryQueryOptions('media', noEarlyStart));
		assert.equal(data.items.length, 10_000);
		assert.equal(listRequests, 1);
		assert.equal(new URL(requestedUrls[0]).search, '');
	});
});

describe('listing from the shell', () => {
	let asked: string[] = [];
	const source = (early: FileList | null): EarlyStartSource => ({
		takeSession: async () => null,
		takeDirectory: async (url) => {
			asked.push(url);
			return early;
		}
	});

	beforeEach(() => {
		asked = [];
	});

	test('is used as it is, with no request of its own', async () => {
		const data = await new QueryClient().fetchQuery(
			directoryQueryOptions('media', source(listing('early.txt')))
		);
		assert.equal(data.items[0].name, 'early.txt');
		assert.equal(listRequests, 0);
		assert.match(asked[0], /\/files\/list\/media$/);
	});

	test('falls back to the network when the shell has nothing usable', async () => {
		const data = await new QueryClient().fetchQuery(directoryQueryOptions('media', source(null)));
		assert.equal(data.items[0].name, 'a.txt');
		assert.equal(listRequests, 1);
	});
});
