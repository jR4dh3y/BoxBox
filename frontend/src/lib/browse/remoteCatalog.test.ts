import assert from 'node:assert/strict';
import { afterEach, beforeEach, test } from 'node:test';
import { QueryObserver } from '@tanstack/svelte-query';
import { browseRequests } from './requests.svelte';
import { remoteCatalogClient, remoteCatalogKey } from './remoteCatalog';

beforeEach(() => {
	browseRequests.reset();
	browseRequests.startSession();
});
afterEach(() => browseRequests.reset());

const settle = () => new Promise((resolve) => setTimeout(resolve, 20));

test('remote catalog invalidation waits for a confirming response after an already pending snapshot', async () => {
	let release = () => {};
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	let calls = 0;
	const observer = new QueryObserver(remoteCatalogClient, {
		queryKey: [...remoteCatalogKey, 'remote', 'fixture'],
		queryFn: async () => {
			const snapshot = ++calls;
			if (snapshot === 1) await gate;
			return snapshot;
		}
	});
	const stop = observer.subscribe(() => {});
	try {
		await settle();
		let confirmed = false;
		const changing = Promise.all([browseRequests.changed(), browseRequests.changed()]).then(() => {
			confirmed = true;
		});
		assert.equal(confirmed, false);
		release();
		await changing;
		assert.equal(calls, 2, 'a burst behind the pending response queues one confirmation');
		assert.equal(observer.getCurrentResult().data, 2);
	} finally {
		release();
		stop();
	}
});

test('session reset cancels the lazy remote catalog and prevents late publication into the next session', async () => {
	let release = () => {};
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	let signal: AbortSignal | undefined;
	const key = [...remoteCatalogKey, 'remote', 'fixture'];
	const observer = new QueryObserver(remoteCatalogClient, {
		queryKey: key,
		queryFn: async (context) => {
			signal = context.signal;
			await gate;
			return 'private-old-session';
		}
	});
	const stop = observer.subscribe(() => {});
	try {
		await settle();
		browseRequests.reset();
		assert.equal(signal?.aborted, true);
		browseRequests.startSession();
		await remoteCatalogClient.fetchQuery({ queryKey: key, queryFn: async () => 'new-session' });
		release();
		await settle();
		assert.equal(remoteCatalogClient.getQueryData(key), 'new-session');
	} finally {
		release();
		stop();
	}
});
