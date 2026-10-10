import assert from 'node:assert/strict';
import { afterEach, beforeEach, describe, test } from 'node:test';
import { InfiniteQueryObserver, QueryClient } from '@tanstack/svelte-query';
import { CONFIG } from '$lib/config';
import { directoryQueryOptions } from './directoryQuery';

const realFetch = globalThis.fetch;
const realWindow = globalThis.window;
const options = { pageSize: 50, sortBy: 'name', sortDir: 'asc' } as const;
let listRequests = 0;
let names: string[] = [];

beforeEach(() => {
	listRequests = 0;
	names = ['a.txt'];
	Object.assign(globalThis, {
		// The API client builds its URLs from the page origin.
		window: { location: { origin: 'http://localhost' } },
		fetch: async () => {
			listRequests++;
			const items = names.map((name) => ({ name, path: `media/${name}`, isDir: false }));
			return Response.json({
				path: 'media',
				items,
				totalCount: items.length,
				page: 1,
				pageSize: 50
			});
		}
	});
});

afterEach(() => Object.assign(globalThis, { fetch: realFetch, window: realWindow }));

const settle = () => new Promise((resolve) => setTimeout(resolve, 20));

describe('open folder freshness', () => {
	test('a hover prefetch fills the cache once, however often the pointer returns', async () => {
		const queryClient = new QueryClient();
		const prefetch = () =>
			queryClient.prefetchInfiniteQuery({
				...directoryQueryOptions('media', options),
				staleTime: CONFIG.query.prefetchStaleTimeMs
			});
		await prefetch();
		await prefetch();
		assert.equal(listRequests, 1);
	});

	test('opening a prefetched folder shows its page at once, then confirms it with the server', async () => {
		const queryClient = new QueryClient();
		await queryClient.prefetchInfiniteQuery({
			...directoryQueryOptions('media', options),
			staleTime: CONFIG.query.prefetchStaleTimeMs
		});
		names = ['a.txt', 'added-outside.txt']; // a change made after the prefetch

		const observer = new InfiniteQueryObserver(
			queryClient,
			directoryQueryOptions('media', options)
		);
		const unsubscribe = observer.subscribe(() => {});
		const atOnce = observer.getCurrentResult().data?.pages[0].items.map((item) => item.name);
		assert.deepEqual(atOnce, ['a.txt'], 'the cached page must not wait for the network');

		await settle();
		assert.equal(listRequests, 2, 'the open folder must always ask the server');
		const confirmed = observer.getCurrentResult().data?.pages[0].items.map((item) => item.name);
		assert.deepEqual(confirmed, ['a.txt', 'added-outside.txt']);
		unsubscribe();
	});
});
