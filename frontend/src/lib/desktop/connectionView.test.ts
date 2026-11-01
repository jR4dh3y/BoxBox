import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { connectionSummary, connectionView, serverAddress } from './connectionView';

const now = Date.parse('2026-10-28T12:00:00Z');
const server = {
	id: 'a',
	name: 'NAS',
	url: 'https://fm.example.com/',
	expiresAt: '2026-11-27T00:00:00Z'
};

describe('connection view', () => {
	test('a probed server shows its drives and expiry, with no repair action', () => {
		const view = connectionView(server, { kind: 'ready', paths: [{ name: 'a', path: 'a' }] }, now);
		assert.equal(view.kind, 'connected');
		assert.match(connectionSummary(view), /^1 drive · access until /);
	});

	test('an unprobed server is checking', () => {
		assert.equal(connectionView(server, undefined, now).kind, 'checking');
	});

	test('an expired pairing needs pairing whatever the last probe said', () => {
		const view = connectionView(
			{ ...server, expiresAt: '2026-10-01T00:00:00Z' },
			{ kind: 'ready', paths: [] },
			now
		);
		assert.deepEqual(view, { kind: 'needs-pairing', summary: 'Access expired' });
	});

	test('a revoked credential needs pairing; other failures are offline and retryable', () => {
		assert.equal(
			connectionView(server, { kind: 'failed', message: 'x', needsPairing: true }, now).kind,
			'needs-pairing'
		);
		assert.deepEqual(
			connectionView(
				server,
				{ kind: 'failed', message: 'Remote server unavailable', needsPairing: false },
				now
			),
			{ kind: 'offline', detail: 'Remote server unavailable' }
		);
	});

	test('the address drops the scheme and a bare trailing slash', () => {
		assert.equal(serverAddress('https://fm.example.com/'), 'fm.example.com');
		assert.equal(serverAddress('https://example.com:8443/boxbox/'), 'example.com:8443/boxbox');
		assert.equal(serverAddress('not a url'), 'not a url');
	});
});
