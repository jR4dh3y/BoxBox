import assert from 'node:assert/strict';
import { test } from 'node:test';
import { selectedFileBytes, useBrowseSelection, withPathSelected } from './selection.svelte';

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

test('selected size counts files only', () => {
	const items = [
		{ path: 'a', isDir: false, size: 10 },
		{ path: 'b', isDir: false, size: 5 },
		{ path: 'c', isDir: true, size: 4096 }
	];
	assert.equal(selectedFileBytes(items, new Set(['a', 'c'])), 10);
	assert.equal(selectedFileBytes(items, new Set()), 0);
});

test('a checkbox changes only its own path', () => {
	const start = new Set(['a', 'c']);
	assert.deepEqual([...withPathSelected(start, 'b', true)], ['a', 'c', 'b']);
	assert.deepEqual([...withPathSelected(start, 'c', false)], ['a']);
	assert.deepEqual([...start], ['a', 'c']);
});
