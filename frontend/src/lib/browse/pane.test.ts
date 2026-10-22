import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { parseBrowseLaunchState, createBrowsePaneState } from './pane.svelte';

describe('browse launch state', () => {
	test('revealing hidden files stays with the pane until it navigates', () => {
		const initial = parseBrowseLaunchState(
			new URL('http://boxbox.local/browse?path=home')
		).location;
		const pane = createBrowsePaneState('test', initial);
		pane.showHiddenFiles();
		assert.equal(pane.revealHiddenFiles, true);
		pane.location.navigate('home/documents');
		assert.equal(pane.revealHiddenFiles, false);
	});
	test('uses the selected file parent as the initial folder', () => {
		assert.deepEqual(
			parseBrowseLaunchState(
				new URL('http://boxbox.local/browse?path=old&select=media%2Fmovies%2Ffilm.mkv')
			),
			{
				location: {
					path: 'media/movies',
					searchQuery: '',
					sortBy: 'name',
					sortDir: 'asc',
					viewMode: 'list'
				},
				selectPath: 'media/movies/film.mkv'
			}
		);
	});

	test('does not create a selection for an empty or parent-traversing target', () => {
		assert.equal(
			parseBrowseLaunchState(new URL('http://boxbox.local/browse?select=')).selectPath,
			null
		);
		assert.equal(
			parseBrowseLaunchState(new URL('http://boxbox.local/browse?select=media%2F..%2Fsecret'))
				.selectPath,
			null
		);
	});
});
