import assert from 'node:assert/strict';
import { test } from 'node:test';
import { generatePairingCode, listPairedClients, pairingStatus } from './connect';
import { setAccessToken, clearTokens } from './client';
import { AccountPairing } from '../settings/pairing.svelte';

test('account pairing authorizes, validates codes, and clears passwords', async () => {
	let response: unknown = {
		code: 'ABCD-EFGH',
		expiresAt: new Date(Date.now() + 300_000).toISOString()
	};
	let status = 200;
	let attempts = 0;
	let plainText = false;
	const server = Bun.serve({
		port: 0,
		async fetch(request) {
			attempts++;
			assert.equal(new URL(request.url).pathname, '/api/v1/connect/pairing');
			assert.equal(request.method, 'POST');
			assert.equal(request.headers.get('authorization'), 'Bearer test-account');
			assert.deepEqual(await request.json(), { password: 'confirm-password' });
			return plainText
				? new Response('Method not allowed', { status })
				: Response.json(response, { status });
		}
	});
	setAccessToken('test-account');
	const previous = globalThis.window;
	Object.assign(globalThis, { window: { location: { origin: server.url.origin } } });
	const pairing = new AccountPairing();
	try {
		pairing.password = 'confirm-password';
		await pairing.generate();
		assert.equal(pairing.password, '');
		assert.equal(pairing.state.kind, 'ready');
		assert.equal(attempts, 1);
		for (const invalid of [
			null,
			{},
			{ code: 'wrong', expiresAt: new Date(Date.now() + 300_000).toISOString() },
			{ code: 'ABCD-EFGH', expiresAt: 'invalid' }
		]) {
			response = invalid;
			await assert.rejects(generatePairingCode('confirm-password'), /invalid pairing code/);
		}
		status = 429;
		response = { error: 'Too many attempts. Try again later.', code: 'RATE_LIMITED' };
		pairing.password = 'confirm-password';
		const before = attempts;
		await pairing.generate();
		assert.equal(attempts, before + 1);
		assert.equal(pairing.password, '');
		assert.equal(pairing.state.kind, 'idle');
		assert.match(pairing.error, /Too many attempts/);
		for (const unsupportedStatus of [404, 405]) {
			status = unsupportedStatus;
			plainText = true;
			await assert.rejects(generatePairingCode('confirm-password'), /Update the server/);
		}
	} finally {
		pairing.dispose();
		Object.assign(globalThis, { window: previous });
		clearTokens();
		server.stop(true);
	}
});

test('server clients reconcile redeemed codes and revoke with an empty 204 response', async () => {
	const device = {
		clientId: 'client/one',
		clientName: 'Test desktop',
		username: 'test',
		createdAt: new Date().toISOString(),
		expiresAt: new Date(Date.now() + 300_000).toISOString()
	};
	let clients: unknown = { clients: [device] };
	let enabled = true;
	let revocations = 0;
	const server = Bun.serve({
		port: 0,
		async fetch(request) {
			const path = new URL(request.url).pathname;
			assert.equal(request.headers.get('authorization'), 'Bearer test-account');
			if (request.method === 'DELETE') {
				assert.equal(path, '/api/v1/connect/clients/client%2Fone');
				revocations++;
				clients = { clients: [] };
				return new Response(null, { status: 204 });
			}
			if (path.endsWith('/clients')) return Response.json(clients);
			if (request.method === 'POST')
				return Response.json({ code: 'ABCD-EFGH', expiresAt: device.expiresAt });
			return Response.json(
				enabled ? { enabled: true, expiresAt: device.expiresAt } : { enabled: false }
			);
		}
	});
	setAccessToken('test-account');
	const previous = globalThis.window;
	Object.assign(globalThis, { window: { location: { origin: server.url.origin } } });
	const controller = new AccountPairing();
	try {
		assert.deepEqual(await listPairedClients(), [device]);
		assert.deepEqual(await pairingStatus(), { enabled: true, expiresAt: device.expiresAt });
		controller.password = 'test-password';
		await controller.generate();
		await controller.refresh();
		assert.equal(controller.state.kind, 'ready');
		enabled = false;
		await controller.refresh();
		assert.equal(controller.state.kind, 'closed');
		assert.deepEqual(controller.clients, { kind: 'ready', clients: [device] });
		await controller.revoke(device.clientId);
		assert.equal(revocations, 1);
		assert.deepEqual(controller.clients, { kind: 'ready', clients: [] });
		clients = { clients: [{ ...device, expiresAt: 'invalid' }] };
		await assert.rejects(listPairedClients(), /timestamp/);
	} finally {
		controller.dispose();
		Object.assign(globalThis, { window: previous });
		clearTokens();
		server.stop(true);
	}
});

test('a rejected confirmation password is submitted once without refreshing or replaying', async () => {
	let attempts = 0;
	let refreshes = 0;
	const server = Bun.serve({
		port: 0,
		async fetch(request) {
			if (new URL(request.url).pathname === '/api/v1/auth/refresh') {
				refreshes++;
				return Response.json({
					accessToken: 'refreshed-account',
					expiresAt: '2030-01-01T00:00:00Z'
				});
			}
			attempts++;
			assert.equal(request.headers.get('authorization'), 'Bearer test-account');
			assert.deepEqual(await request.json(), { password: 'wrong-password' });
			return Response.json({ error: 'Invalid credentials', code: 'UNAUTHORIZED' }, { status: 401 });
		}
	});
	setAccessToken('test-account');
	const previous = globalThis.window;
	Object.assign(globalThis, { window: { location: { origin: server.url.origin } } });
	const controller = new AccountPairing();
	try {
		controller.password = 'wrong-password';
		await controller.generate();
		assert.equal(attempts, 1);
		assert.equal(refreshes, 0);
		assert.equal(controller.password, '');
		assert.equal(controller.state.kind, 'idle');
		assert.equal(controller.error, 'Invalid credentials');
	} finally {
		controller.dispose();
		Object.assign(globalThis, { window: previous });
		clearTokens();
		server.stop(true);
	}
});

test('server pairing status governs expiry with browser clocks ahead and behind', async () => {
	const serverNow = Date.parse('2030-01-01T12:00:00Z');
	const expiresAt = new Date(serverNow + 300_000).toISOString();
	let enabled = true;
	const server = Bun.serve({
		port: 0,
		fetch(request) {
			const headers = { Date: new Date(serverNow).toUTCString() };
			if (request.method === 'POST')
				return Response.json({ code: 'ABCD-EFGH', expiresAt }, { headers });
			if (new URL(request.url).pathname.endsWith('/clients'))
				return Response.json({ clients: [] }, { headers });
			return Response.json(enabled ? { enabled: true, expiresAt } : { enabled: false }, {
				headers
			});
		}
	});
	setAccessToken('test-account');
	const previous = globalThis.window;
	const originalNow = Date.now;
	Object.assign(globalThis, { window: { location: { origin: server.url.origin } } });
	const controller = new AccountPairing();
	try {
		for (const offset of [600_000, -600_000]) {
			Date.now = () => serverNow + offset;
			enabled = true;
			controller.password = 'confirm-password';
			await controller.generate();
			await new Promise<void>((resolve) => setTimeout(resolve, 0));
			assert.equal(
				controller.state.kind,
				'ready',
				'a client clock must not expire a server-issued code'
			);
			await controller.refresh();
			assert.equal(controller.state.kind, 'ready');
			enabled = false;
			await controller.refresh();
			assert.equal(
				controller.state.kind,
				'closed',
				'server expiry must hide the code even when the browser clock is behind'
			);
		}
	} finally {
		Date.now = originalNow;
		controller.dispose();
		Object.assign(globalThis, { window: previous });
		clearTokens();
		server.stop(true);
	}
});
