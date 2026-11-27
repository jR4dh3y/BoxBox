import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
	parseBrowseLocation,
	usePaneLocation,
	type BrowseLocationSnapshot
} from './paneLocation.svelte';

const initial: BrowseLocationSnapshot = {
	page: 'files',
	path: 'media',
	searchQuery: '',
	sortBy: 'name',
	sortDir: 'asc',
	viewMode: 'list'
};

describe('browse location', () => {
	test('remote root is the upper bound of pane history and parent navigation', () => {
		const root = '@remote/aaaabbbb-cccc-4ddd-8eee-ffffffffffff';
		const location = usePaneLocation({ initial: { ...initial, path: root } });
		assert.equal(location.canGoUp, false);
		location.navigate(`${root}/media/folder`);
		location.up();
		assert.equal(location.path, `${root}/media`);
		location.up();
		location.up();
		assert.equal(location.path, root);
		location.back();
		assert.equal(location.path, `${root}/media`);
	});

	test('browser navigation restores a pane history entry without adding another one', () => {
		const location = usePaneLocation({ initial });
		location.navigate('media/a');
		const first = location.snapshot;
		location.navigate('media/b');
		location.restore(first);
		assert.equal(location.path, 'media/a');
		assert.equal(location.canGoForward, true);
		location.forward();
		assert.equal(location.path, 'media/b');
	});

	test('folder navigation pushes history while search, sort, and view replace it', () => {
		const location = usePaneLocation({ initial });
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
});

test('a malformed remote launch URL falls back to the root', () => {
	const location = parseBrowseLocation(
		new URL('https://boxbox.test/browse?path=%40remote%2Fbroken')
	);
	assert.equal(location.path, '');
});
