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
	test('fresh panes use newest first while explicit URL sorts win', () => {
		const defaultLocation = parseBrowseLocation(new URL('http://boxbox.local/browse?path=media'));
		assert.equal(defaultLocation.sortBy, 'modTime');
		assert.equal(defaultLocation.sortDir, 'desc');
		const explicit = parseBrowseLocation(new URL('http://boxbox.local/browse?sort=name&dir=asc'));
		assert.equal(explicit.sortBy, 'name');
		assert.equal(explicit.sortDir, 'asc');
	});
	test('This PC defaults to master drive cards while folders retain display preferences', () => {
		const root = usePaneLocation({
			initial: parseBrowseLocation(new URL('http://boxbox.local/browse'))
		});
		assert.equal(root.driveViewMode, 'grid');
		root.navigate('media');
		assert.equal(root.viewMode, 'list');
		root.navigate('');
		assert.equal(root.driveViewMode, 'grid');
		assert.equal(
			parseBrowseLocation(new URL('http://boxbox.local/browse?view=list')).viewMode,
			'list'
		);
	});
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
	test('parses the URL into a typed pane snapshot', () => {
		assert.deepEqual(
			parseBrowseLocation(
				new URL(
					'http://boxbox.local/browse?path=%2Fmedia%2Fmovies%2F&q=%20cats%20&sort=size&dir=desc&view=grid'
				)
			),
			{
				page: 'files',
				path: 'media/movies',
				searchQuery: ' cats ',
				sortBy: 'size',
				sortDir: 'desc',
				viewMode: 'grid'
			}
		);
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

	test('up and a new path discard forward history', () => {
		const location = usePaneLocation({ initial: { ...initial, path: 'media/movies/2026' } });
		location.up();
		assert.equal(location.path, 'media/movies');
		location.back();
		location.navigate('media/photos');
		assert.equal(location.path, 'media/photos');
		assert.equal(location.canGoForward, false);
	});
});

test('a malformed remote launch URL falls back to the root', () => {
	const location = parseBrowseLocation(
		new URL('https://boxbox.test/browse?path=%40remote%2Fbroken')
	);
	assert.equal(location.path, '');
});
