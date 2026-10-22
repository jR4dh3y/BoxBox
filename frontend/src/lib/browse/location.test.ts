import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
	parseBrowseLocation,
	useBrowseLocation,
	type BrowseLocationSnapshot
} from './location.svelte';

const initial: BrowseLocationSnapshot = {
	path: 'media',
	searchQuery: '',
	sortBy: 'name',
	sortDir: 'asc',
	viewMode: 'list'
};

describe('browse location', () => {
	test('browser navigation restores a pane history entry without adding another one', () => {
		const location = useBrowseLocation({ initial });
		location.navigate('media/a');
		const first = location.snapshot;
		location.navigate('media/b');
		location.restore(first);
		assert.equal(location.path, 'media/a');
		assert.equal(location.canGoForward, true);
		location.forward();
		assert.equal(location.path, 'media/b');
	});
	test('parses the URL into a typed pane snapshot', () => {
		assert.deepEqual(
			parseBrowseLocation(
				new URL(
					'http://boxbox.local/browse?path=%2Fmedia%2Fmovies%2F&q=%20cats%20&sort=size&dir=desc&view=grid'
				)
			),
			{
				path: 'media/movies',
				searchQuery: ' cats ',
				sortBy: 'size',
				sortDir: 'desc',
				viewMode: 'grid'
			}
		);
	});

	test('folder navigation pushes history while search, sort, and view replace it', () => {
		const location = useBrowseLocation({ initial });
		location.navigate('media/movies');
		location.search('  cats  ');
		location.setSort('size', 'desc');
		location.setViewMode('grid');

		assert.equal(location.path, 'media/movies');
		assert.equal(location.searchQuery, 'cats');
		assert.equal(location.sortBy, 'size');
		assert.equal(location.sortDir, 'desc');
		assert.equal(location.viewMode, 'grid');
		location.back();
		assert.equal(location.path, 'media');
		assert.equal(location.canGoForward, true);
		location.forward();
		assert.equal(location.path, 'media/movies');
		assert.equal(location.viewMode, 'grid');
	});

	test('up and a new path discard forward history', () => {
		const location = useBrowseLocation({ initial: { ...initial, path: 'media/movies/2026' } });
		location.up();
		assert.equal(location.path, 'media/movies');
		location.back();
		location.navigate('media/photos');
		assert.equal(location.path, 'media/photos');
		assert.equal(location.canGoForward, false);
	});
});
