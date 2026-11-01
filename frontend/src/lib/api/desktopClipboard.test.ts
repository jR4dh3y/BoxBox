import assert from 'node:assert/strict';
import { afterEach, beforeEach, test } from 'node:test';
import {
	clearDesktopCutClipboard,
	getDesktopClipboard,
	setDesktopClipboard
} from './desktopClipboard';

const realFetch = globalThis.fetch;
const requests: Array<{ url: string; method: string; body: string | undefined }> = [];

type FetchImplementation = (
	input: Parameters<typeof fetch>[0],
	init?: Parameters<typeof fetch>[1]
) => ReturnType<typeof fetch>;

function installFetchMock(implementation: FetchImplementation): void {
	globalThis.fetch = Object.assign(implementation, { preconnect: realFetch.preconnect });
}

const realWindow = globalThis.window;
const realLocalStorage = globalThis.localStorage;

beforeEach(() => {
	const storage = new Map<string, string>();
	Object.assign(globalThis, {
		window: Object.assign(new EventTarget(), { location: { origin: 'http://desktop.test' } }),
		localStorage: {
			getItem: (key: string) => storage.get(key) ?? null,
			setItem: (key: string, value: string) => storage.set(key, value),
			removeItem: (key: string) => storage.delete(key)
		}
	});
});

afterEach(() => {
	requests.length = 0;
	globalThis.fetch = realFetch;
	Object.assign(globalThis, { window: realWindow, localStorage: realLocalStorage });
});

test('desktop clipboard API parses typed file items and null selections', async () => {
	installFetchMock(async (input, init) => {
		requests.push({
			url: new URL(String(input)).pathname,
			method: init?.method ?? 'GET',
			body: typeof init?.body === 'string' ? init.body : undefined
		});
		return Response.json({
			operation: 'cut',
			items: [{ path: 'home/report.txt', name: 'report.txt', isDir: false }]
		});
	});
	const contents = await getDesktopClipboard();
	assert.deepEqual(contents, {
		operation: 'cut',
		items: [{ path: 'home/report.txt', name: 'report.txt', isDir: false }]
	});

	installFetchMock(async () => Response.json(null));
	assert.equal(await getDesktopClipboard(), null);
});

test('desktop clipboard writes file paths and guards cut clearing by operation', async () => {
	installFetchMock(async (input, init) => {
		requests.push({
			url: new URL(String(input)).pathname,
			method: init?.method ?? 'GET',
			body: typeof init?.body === 'string' ? init.body : undefined
		});
		return Response.json({ ok: true });
	});
	await setDesktopClipboard(['home/report.txt'], 'cut');
	await clearDesktopCutClipboard(['home/report.txt']);
	assert.deepEqual(requests, [
		{
			url: '/api/v1/desktop/clipboard',
			method: 'POST',
			body: JSON.stringify({ operation: 'cut', paths: ['home/report.txt'] })
		},
		{
			url: '/api/v1/desktop/clipboard',
			method: 'POST',
			body: JSON.stringify({ operation: 'clear-cut', paths: ['home/report.txt'] })
		}
	]);
});

test('desktop clipboard rejects malformed external JSON', async () => {
	installFetchMock(async () =>
		Response.json({
			operation: 'cut',
			items: [{ path: 'home/item', name: 'item', isDir: 'false' }]
		})
	);
	await assert.rejects(getDesktopClipboard(), /Invalid desktop clipboard item/);
});
