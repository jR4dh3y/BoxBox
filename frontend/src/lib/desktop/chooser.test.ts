import { test } from 'node:test';
import assert from 'node:assert/strict';
import { globMatch } from './chooser.svelte';
test('portal globs cover wildcards, case classes and literal regex characters', () => {
	assert.equal(globMatch('*.[pP][nN][gG]', 'image.PNG'), true);
	assert.equal(globMatch('Report?.txt', 'Report1.txt'), true);
	assert.equal(globMatch('data(1).txt', 'data(1).txt'), true);
	assert.equal(globMatch('*.txt', 'image.png'), false);
	assert.equal(globMatch('[!a]*', 'beta'), true);
});
