import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ChooserServers } from './chooserServers.svelte';
import type { MountPoint } from '#lib/api/files.js';

const online = { id: 'online', name: 'Home server', url: 'https://home.test', expiresAt: '' };
const offline = { ...online, id: 'offline', name: 'Offline server' };
const root: MountPoint = {
	name: '@remote/online/photos',
	path: '@remote/online/photos',
	label: 'photos',
	readOnly: false
};

test('server roots publish independently while another server is still loading, and retry failures', async () => {
	let release: (value: { roots: MountPoint[] }) => void = () => {};
	const delayed = new Promise<{ roots: MountPoint[] }>((resolve) => {
		release = resolve;
	});
	let fails = true;
	const catalog = new ChooserServers(
		async () => [online, offline],
		async (server) => {
			if (server?.id === 'online') return delayed;
			if (fails) throw new Error('Pair this server again');
			return { roots: [root] };
		}
	);
	const loading = catalog.load(new AbortController().signal);
	await new Promise((resolve) => setTimeout(resolve, 0));
	const entries = catalog.catalog.value;
	assert.ok(entries);
	assert.equal(entries[0].roots.state.kind, 'loading');
	assert.deepEqual(entries[1].roots.state, { kind: 'failed', message: 'Pair this server again' });
	fails = false;
	await entries[1].roots.load();
	assert.deepEqual(entries[1].roots.value, [root]);
	assert.equal(entries[0].roots.state.kind, 'loading');
	release({ roots: [root] });
	await loading;
	assert.deepEqual(entries[0].roots.value, [root]);
});

test('closing a picker before saved servers arrive does not start upstream discovery', async () => {
	let release: (value: (typeof online)[]) => void = () => {};
	let requests = 0;
	const catalog = new ChooserServers(
		() =>
			new Promise((resolve) => {
				release = resolve;
			}),
		async () => {
			requests++;
			return { roots: [] };
		}
	);
	const abort = new AbortController();
	const loading = catalog.load(abort.signal);
	abort.abort();
	release([online]);
	await loading;
	assert.equal(requests, 0);
});
