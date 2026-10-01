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
		for (let attempt = 0; attempt < 8; attempt++) {
			FakeSocket.instances.at(-1)?.onclose?.({ code: 1006 });
			scheduled.shift()?.();
		}

		const { initialReconnectDelayMs: first, maxReconnectDelayMs: max } = CONFIG.websocket;
		assert.deepEqual(
			delays,
			Array.from({ length: 8 }, (_, attempt) => Math.min(first * 2 ** attempt, max))
		);
	});

	test('a clean close does not reconnect, and an open connection resets the delay', () => {
		websocketStore.connect(true);
		const [socket] = FakeSocket.instances;
		socket.onclose?.({ code: 1006 });
		scheduled.shift()?.();
		FakeSocket.instances.at(-1)?.onopen?.();
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
