import assert from 'node:assert/strict';
import { afterEach, beforeEach, describe, test } from 'node:test';
import { CONFIG } from '$lib/config';
import { connectFromEffect, flushSync, websocketStore } from './websocket.test.svelte';

class FakeSocket {
	static instances: FakeSocket[] = [];
	static readonly OPEN = 1;
	readyState = 0;
	onopen: (() => void) | null = null;
	onclose: ((event: { code: number }) => void) | null = null;
	onerror: (() => void) | null = null;
	onmessage: ((event: unknown) => void) | null = null;
	constructor(readonly url: string) {
		FakeSocket.instances.push(this);
	}
	send() {}
	close() {}
}

const realSetTimeout = globalThis.setTimeout;
const realSocket = globalThis.WebSocket;
const realWindow = globalThis.window;
let delays: number[] = [];
let scheduled: (() => void)[] = [];

beforeEach(() => {
	FakeSocket.instances = [];
	delays = [];
	scheduled = [];
	Object.assign(globalThis, {
		window: { location: { protocol: 'http:', host: 'localhost' } },
		WebSocket: FakeSocket,
		// Record the delay and let the test decide when the retry fires.
		setTimeout: (callback: () => void, delay: number) => {
			delays.push(delay);
			scheduled.push(callback);
			return delays.length;
		}
	});
});

afterEach(() => {
	websocketStore.disconnect();
	Object.assign(globalThis, {
		setTimeout: realSetTimeout,
		WebSocket: realSocket,
		window: realWindow
	});
});

/** Drops the newest socket, then runs the retry the store scheduled. Returns the new socket. */
function dropAndRetry(): FakeSocket {
	const before = FakeSocket.instances.length;
	FakeSocket.instances.at(-1)?.onclose?.({ code: 1006 });
	scheduled.shift()?.();
	assert.equal(FakeSocket.instances.length, before + 1, 'the retry must open a new socket');
	return FakeSocket.instances[before];
}

describe('websocket reconnect backoff', () => {
	test('a connect() call from an effect does not retry on its own state changes', () => {
		const stop = connectFromEffect();
		flushSync();
		assert.equal(FakeSocket.instances.length, 1);

		FakeSocket.instances[0].onclose?.({ code: 1006 });
		flushSync();

		assert.equal(websocketStore.connectionState, 'reconnecting');
		assert.equal(FakeSocket.instances.length, 1, 'no socket may open before the delay passes');
		assert.deepEqual(delays, [CONFIG.websocket.initialReconnectDelayMs]);
		stop();
	});

	test('delays double on each failed attempt and stop at the maximum', () => {
		websocketStore.connect(true);
		for (let attempt = 0; attempt < 8; attempt++) dropAndRetry();
		FakeSocket.instances.at(-1)?.onclose?.({ code: 1006 });

		const { initialReconnectDelayMs: first, maxReconnectDelayMs: max } = CONFIG.websocket;
		assert.deepEqual(
			delays,
			Array.from({ length: 9 }, (_, attempt) => Math.min(first * 2 ** attempt, max))
		);
	});

	test('retries stop at the configured attempt limit', () => {
		const { maxReconnectAttempts } = CONFIG.websocket;
		websocketStore.connect(true);
		for (let attempt = 0; attempt < maxReconnectAttempts; attempt++) dropAndRetry();

		const sockets = FakeSocket.instances.length;
		FakeSocket.instances.at(-1)?.onclose?.({ code: 1006 });

		assert.equal(delays.length, maxReconnectAttempts, 'no retry may be scheduled past the limit');
		assert.equal(scheduled.length, 0);
		assert.equal(FakeSocket.instances.length, sockets);
		assert.equal(websocketStore.connectionState, 'disconnected');
		assert.equal(websocketStore.error, 'Max reconnection attempts reached');
	});

	test('retries keep the connection mode, so a dev server without a token reconnects', () => {
		websocketStore.connect(true);
		dropAndRetry();
		dropAndRetry();
		assert.equal(websocketStore.connectionState, 'connecting');
	});

	test('an open connection resets the delay, and a clean close does not reconnect', () => {
		websocketStore.connect(true);
		dropAndRetry().onopen?.();
		FakeSocket.instances.at(-1)?.onclose?.({ code: 1006 });
		assert.deepEqual(delays, [
			CONFIG.websocket.initialReconnectDelayMs,
			CONFIG.websocket.initialReconnectDelayMs
		]);

		scheduled.shift()?.();
		const before = delays.length;
		FakeSocket.instances.at(-1)?.onclose?.({ code: 1000 });
		assert.equal(delays.length, before);
		assert.equal(websocketStore.connectionState, 'disconnected');
	});
});

describe('websocket reconnect job refresh', () => {
	const realFetch = globalThis.fetch;
	let jobRequests: string[] = [];

	beforeEach(() => {
		jobRequests = [];
		Object.assign(globalThis, {
			// The API client builds its URLs from the page origin.
			window: { location: { protocol: 'http:', host: 'localhost', origin: 'http://localhost' } },
			fetch: async (input: string) => {
				jobRequests.push(input);
				return Response.json({ jobs: [] });
			}
		});
	});

	afterEach(() => {
		Object.assign(globalThis, { fetch: realFetch });
	});

	/** `loadJobs` reaches `fetch` after a few awaits. */
	const settle = () => new Promise((resolve) => realSetTimeout(resolve, 0));

	test('reloads jobs after a dropped connection comes back, not on the first open', async () => {
		websocketStore.connect(true);
		FakeSocket.instances[0].onopen?.();
		await settle();
		assert.deepEqual(jobRequests, []);

		dropAndRetry().onopen?.();
		await settle();
		assert.equal(jobRequests.length, 1);
		assert.match(jobRequests[0], /\/jobs$/);
	});

	test('does not reload jobs again when a healthy connection simply stays open', async () => {
		websocketStore.connect(true);
		dropAndRetry().onopen?.();
		await settle();
		const afterReconnect = jobRequests.length;
		FakeSocket.instances.at(-1)?.onmessage?.({ data: '{"type":"pong"}' });
		await settle();
		assert.equal(jobRequests.length, afterReconnect);
	});
});
