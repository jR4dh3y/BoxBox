import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import { clearBrowseCutClipboardAfterMove, writeBrowseClipboard } from './desktopClipboard';

const realFetch = globalThis.fetch;
const requests: Array<{ body: string | undefined }> = [];

type FetchImplementation = (
	input: Parameters<typeof fetch>[0],
	init?: Parameters<typeof fetch>[1]
) => ReturnType<typeof fetch>;

function installFetchMock(implementation: FetchImplementation): void {
	globalThis.fetch = Object.assign(implementation, { preconnect: realFetch.preconnect });
}

afterEach(() => {
	requests.length = 0;
	globalThis.fetch = realFetch;
});

test('browse clipboard publishes selected paths and leaves copies intact after paste', async () => {
	installFetchMock(async (_input, init) => {
		requests.push({ body: typeof init?.body === 'string' ? init.body : undefined });
		return new Response(null, { status: 204 });
	});
	await writeBrowseClipboard([{ path: 'home/one.txt' }, { path: 'home/two.txt' }], 'copy');
	await clearBrowseCutClipboardAfterMove({
		operation: 'copy',
		items: [{ path: 'home/one.txt', name: 'one.txt', isDir: false }]
	});
	assert.deepEqual(requests, [
		{ body: JSON.stringify({ operation: 'copy', paths: ['home/one.txt', 'home/two.txt'] }) }
	]);
});

test('browse clipboard clears cuts only after a successful move call', async () => {
	installFetchMock(async (_input, init) => {
		requests.push({ body: typeof init?.body === 'string' ? init.body : undefined });
		return new Response(null, { status: 204 });
	});
	await clearBrowseCutClipboardAfterMove({
		operation: 'cut',
		items: [{ path: 'home/moved.txt', name: 'moved.txt', isDir: false }]
	});
	assert.deepEqual(requests, [
		{ body: JSON.stringify({ operation: 'clear-cut', paths: ['home/moved.txt'] }) }
	]);
});
