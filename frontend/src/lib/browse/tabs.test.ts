import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { useBrowseTabs } from './tabs.svelte';
import type { BrowseLocationSnapshot } from './paneLocation.svelte';

const initial: BrowseLocationSnapshot = {
	page: 'files',
	path: 'media',
	searchQuery: '',
	sortBy: 'name',
	sortDir: 'asc',
	viewMode: 'list'
};

describe('browse tabs and panes', () => {
	test('tabs keep separate location, history, and selection state', () => {
		const tabs = useBrowseTabs({ initialLocation: initial });
		const first = tabs.activeTab;
		assert.ok(first);
		const firstPane = first.panes[0];
		firstPane.location.navigate('media/movies');
		firstPane.selection.set(new Set(['media/movies/film.mkv']));

		const secondId = tabs.newTab();
		assert.equal(tabs.activePane?.location.path, 'media/movies');
		assert.equal(tabs.activePane?.selection.count, 0);
		tabs.activePane?.location.navigate('media/photos');
		tabs.activateTab(first.id);

		assert.equal(tabs.activePane?.location.path, 'media/movies');
		assert.deepEqual([...firstPane.selection.paths], ['media/movies/film.mkv']);
		firstPane.location.back();
		assert.equal(firstPane.location.path, 'media');
		assert.equal(firstPane.selection.count, 0, 'changing folders clears only that pane selection');
		assert.equal(tabs.tabs.length, 2);

		tabs.closeTab(secondId);
		assert.equal(tabs.tabs.length, 1);
		assert.equal(tabs.activeTabId, first.id);
		tabs.closeTab(first.id);
		assert.equal(tabs.tabs.length, 1, 'the last browse tab remains open');
	});

	test('split panes navigate independently and focus selects the toolbar location', () => {
		const activeLocations: string[] = [];
		const tabs = useBrowseTabs({
			initialLocation: initial,
			onActiveLocationChange: (snapshot) => activeLocations.push(snapshot.path)
		});
		const left = tabs.activePane;
		assert.ok(left);
		left.location.navigate('media/left');
		tabs.toggleSplit();
		const right = tabs.activePane;
		assert.ok(right);
		assert.equal(right.id, tabs.activeTab?.panes[1].id);
		assert.equal(right.location.path, 'media/left');
		right.location.navigate('media/right');
		assert.equal(left.location.path, 'media/left');
		assert.equal(tabs.activePane?.location.path, 'media/right');

		tabs.focusPane(left.id);
		assert.equal(tabs.activePane?.location.path, 'media/left');
		const changeCount = activeLocations.length;
		right.location.navigate('media/right-background');
		assert.equal(
			activeLocations.length,
			changeCount,
			'unfocused pane location does not drive shared chrome'
		);
		tabs.focusPane(right.id);
		assert.equal(tabs.activePane?.location.path, 'media/right-background');
		tabs.focusPane(left.id);
		tabs.toggleSplit();
		assert.equal(tabs.hasSplit, false);
		assert.equal(tabs.activePane?.id, left.id);
		assert.equal(tabs.activePane?.location.path, 'media/left');
		assert.ok(activeLocations.includes('media/right'));
		assert.ok(activeLocations.includes('media/left'));
	});

	test('tab cycling wraps and preserves each tab split state', () => {
		const tabs = useBrowseTabs({ initialLocation: initial });
		const firstId = tabs.activeTabId;
		const secondId = tabs.newTab({ ...initial, path: 'media/photos' });
		tabs.toggleSplit();
		tabs.cycleTab(1);
		assert.equal(tabs.activeTabId, firstId);
		assert.equal(tabs.hasSplit, false);
		tabs.cycleTab(-1);
		assert.equal(tabs.activeTabId, secondId);
		assert.equal(tabs.hasSplit, true);
		tabs.cycleTab(1);
		assert.equal(tabs.activeTabId, firstId);
	});
});
