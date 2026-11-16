import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseLoginMethods } from './authConfig';
import { ssoErrorMessage } from '../auth/ssoError';
import { LoginMethodsStore } from '../auth/loginMethods.svelte';

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

test('login methods fall back to the password form when the config cannot be read', async () => {
	const store = new LoginMethodsStore();
	assert.equal(store.state.kind, 'loading');
	await store.load(() => Promise.reject(new Error('404')));
	assert.deepEqual(store.state, { kind: 'ready', methods: { kind: 'password' } });
	await store.load(() => Promise.resolve({ kind: 'sso', providerName: 'SSO' }));
	assert.deepEqual(store.state, { kind: 'ready', methods: { kind: 'sso', providerName: 'SSO' } });
});
