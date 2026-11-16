import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseLoginMethods } from './authConfig';
import { ssoErrorMessage } from '../auth/ssoError';
import { LoginMethodsStore } from '../auth/loginMethods.svelte';
import { ApiRequestError } from './client';

test('auth config parses into the sign-in methods the server accepts', () => {
	assert.deepEqual(parseLoginMethods({ passwordLogin: true, oidc: null }), { kind: 'password' });
	assert.deepEqual(parseLoginMethods({ passwordLogin: true, oidc: { name: 'Authelia' } }), {
		kind: 'password-and-sso',
		providerName: 'Authelia'
	});
	assert.deepEqual(parseLoginMethods({ passwordLogin: false, oidc: { name: 'SSO' } }), {
		kind: 'sso',
		providerName: 'SSO'
	});
	for (const invalid of [
		null,
		[],
		{},
		{ passwordLogin: 'yes', oidc: null },
		{ passwordLogin: false, oidc: null },
		{ passwordLogin: true },
		{ passwordLogin: true, oidc: { name: '' } },
		{ passwordLogin: true, oidc: 'SSO' }
	]) {
		assert.throws(() => parseLoginMethods(invalid), JSON.stringify(invalid));
	}
});

test('sso error codes map to fixed messages', () => {
	assert.equal(ssoErrorMessage(null), null);
	assert.equal(ssoErrorMessage('not_allowed'), "This account isn't allowed to use BoxBox.");
	assert.equal(ssoErrorMessage('denied'), 'Sign-in was cancelled.');
	assert.equal(ssoErrorMessage('<script>'), 'Sign-in failed. Try again.');
	assert.equal(ssoErrorMessage('toString'), 'Sign-in failed. Try again.');
});

test('login methods fall back to password only when the configuration route is missing', async () => {
	const store = new LoginMethodsStore();
	assert.equal(store.state.kind, 'loading');
	await store.load(() => Promise.reject(new ApiRequestError('Not found', 404, 'NOT_FOUND')));
	assert.deepEqual(store.state, { kind: 'ready', methods: { kind: 'password' } });
	await store.load(() => Promise.resolve({ kind: 'sso', providerName: 'SSO' }));
	assert.deepEqual(store.state, { kind: 'ready', methods: { kind: 'sso', providerName: 'SSO' } });
});

test('login methods expose temporary failures and retry without advertising password login', async () => {
	for (const error of [
		new TypeError('Network failed'),
		new ApiRequestError('Unavailable', 503, 'UNAVAILABLE'),
		new Error('Invalid auth config')
	]) {
		const store = new LoginMethodsStore();
		await store.load(() => Promise.reject(error));
		assert.deepEqual(store.state, { kind: 'error' });
		const retry = store.load(() => {
			assert.equal(store.state.kind, 'loading');
			return Promise.resolve({ kind: 'sso', providerName: 'SSO' });
		});
		await retry;
		assert.deepEqual(store.state, { kind: 'ready', methods: { kind: 'sso', providerName: 'SSO' } });
	}
});
