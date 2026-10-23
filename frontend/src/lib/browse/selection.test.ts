import assert from 'node:assert/strict';
import { test } from 'node:test';
import { useBrowseSelection } from './selection.svelte';

const entries = (...paths: string[]) => paths.map((path) => ({ path }));

test('confirmed listing removes vanished selections and retains hidden entries', () => {
	const selection = useBrowseSelection();
	selection.set(new Set(['media/kept.txt', 'media/gone.txt', 'media/.hidden']));
	selection.retain(entries('media/kept.txt', 'media/.hidden', 'media/new.txt'));
	assert.deepEqual([...selection.paths], ['media/kept.txt', 'media/.hidden']);
	const previous = selection.paths;
	selection.retain(entries('media/kept.txt', 'media/.hidden'));
	assert.equal(selection.paths, previous);
	selection.retain([]);
	assert.equal(selection.count, 0);
});
