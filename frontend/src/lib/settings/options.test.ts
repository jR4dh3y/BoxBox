import assert from 'node:assert/strict';
import { test } from 'node:test';
import { SORT_BY_OPTIONS, pickOption } from './options';

test('only a listed value is accepted from a select', () => {
	assert.equal(pickOption(SORT_BY_OPTIONS, 'modTime'), 'modTime');
	assert.equal(pickOption(SORT_BY_OPTIONS, 'Name'), undefined);
});
