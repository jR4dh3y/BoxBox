import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { shellScriptPaths } from './shell-scripts';

describe('shellScriptPaths', () => {
	test('finds a script the shell loads by URL', () => {
		const html = '<head><script src="/early-fetch.js" async></script></head>';
		assert.deepEqual(shellScriptPaths(html), ['early-fetch.js']);
	});

	test('finds it however the attributes are ordered', () => {
		const html = '<script async defer src="/early-fetch.js"></script>';
		assert.deepEqual(shellScriptPaths(html), ['early-fetch.js']);
	});

	test('drops a query string or fragment, which is not part of the file name', () => {
		assert.deepEqual(shellScriptPaths('<script src="/a.js?v=2#x"></script>'), ['a.js']);
	});

	test('leaves out the hashed app chunks, which the manifest already measures', () => {
		const html = '<script src="/_app/immutable/entry/start.abc.js"></script>';
		assert.deepEqual(shellScriptPaths(html), []);
	});

	test('leaves out inline scripts and other origins', () => {
		const html =
			'<script>boot()</script><script src="https://cdn.example/x.js"></script><script src="//cdn.example/y.js"></script>';
		assert.deepEqual(shellScriptPaths(html), []);
	});

	test('finds every one when there are several', () => {
		const html = '<script src="/a.js"></script><link rel="icon"><script src="/b.js"></script>';
		assert.deepEqual(shellScriptPaths(html), ['a.js', 'b.js']);
	});
});
