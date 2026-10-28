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
			{ code: 'ABCD-EFGH', expiresAt: 'invalid' },
			{ code: 'ABCD-EFGH', expiresAt: '2000-01-01T00:00:00Z' }
		]) {
			response = invalid;
			await assert.rejects(generatePairingCode('confirm-password'), /invalid or expired/);
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
