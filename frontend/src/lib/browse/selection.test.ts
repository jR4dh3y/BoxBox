import assert from 'node:assert/strict';
import { test } from 'node:test';
import { useBrowseSelection } from './selection.svelte';

const entries = (...paths: string[]) => paths.map((path) => ({ path }));

test('confirmed listing removes vanished selections and retains hidden entries', () => {
	const selection = useBrowseSelection();
	selection.set(new Set(['media/kept.txt', 'media/gone.txt', 'media/.hidden']));
	selection.retain(entries('media/kept.txt', 'media/.hidden', 'media/new.txt'), 'media');
	assert.deepEqual([...selection.paths], ['media/kept.txt', 'media/.hidden']);
	const previous = selection.paths;
	selection.retain(entries('media/kept.txt', 'media/.hidden'), 'media');
	assert.equal(selection.paths, previous);
	selection.retain([], 'media');
	assert.equal(selection.count, 0);
});

test('a folder listing cannot prune selections from recursive search results', () => {
	const selection = useBrowseSelection();
	selection.set(new Set(['media/gone.txt', 'media/child/kept.txt', 'media-other/kept.txt']));
	selection.retain([], 'media');
	assert.deepEqual([...selection.paths], ['media/child/kept.txt', 'media-other/kept.txt']);
	selection.retain([], 'media/child');
	assert.deepEqual([...selection.paths], ['media-other/kept.txt']);
});

test('root listing prunes only direct children', () => {
	const selection = useBrowseSelection();
	selection.set(new Set(['gone', 'media/kept.txt']));
	selection.retain([], '');
	assert.deepEqual([...selection.paths], ['media/kept.txt']);
});
