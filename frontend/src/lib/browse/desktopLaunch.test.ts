import assert from 'node:assert/strict';
import { test } from 'node:test';
import { openDesktopTab, parseDesktopLaunch, subscribeDesktopLaunches } from './desktopLaunch';
import { useBrowseTabs } from './tabs.svelte';

test('queued and live launch handoffs preserve old tabs, split panes, histories and selection', () => {
	const tabs = useBrowseTabs({
		initialLocation: {
			path: 'root/tmp',
			page: 'files',
			searchQuery: '',
			sortBy: 'name',
			sortDir: 'asc',
			viewMode: 'list'
		}
	});
	const first = tabs.activePane;
	assert.ok(first);
	first.location.navigate('root/tmp/original');
	first.selection.set(new Set(['root/tmp/original/keep.txt']));
	tabs.toggleSplit();
	const originalTab = tabs.activeTab;
	assert.ok(originalTab);
	const source = Object.assign(new EventTarget(), {
		boxboxDesktopTargets: [
			{ path: 'root/tmp/new', select: 'root/tmp/new/selected.txt' },
			{ path: '@remote/invalid' }
		]
	});
	const stop = subscribeDesktopLaunches(source, (target) => openDesktopTab(tabs, target));
	assert.equal(tabs.tabs.length, 2);
	assert.equal(tabs.activePane?.location.path, 'root/tmp/new');
	assert.deepEqual([...(tabs.activePane?.selection.paths ?? [])], ['root/tmp/new/selected.txt']);
	assert.equal(originalTab.panes.length, 2);
	assert.equal(first.location.path, 'root/tmp/original');
	assert.equal(first.location.canGoBack, true);
	assert.deepEqual([...first.selection.paths], ['root/tmp/original/keep.txt']);
	source.boxboxDesktopTargets.push({ path: '@remote/12345678-1234-4234-8234-123456789abc' });
	source.dispatchEvent(new Event('boxbox-desktop-open'));
	assert.equal(tabs.tabs.length, 3);
	stop();
	source.boxboxDesktopTargets.push({ path: 'root/tmp/ignored' });
	source.dispatchEvent(new Event('boxbox-desktop-open'));
	assert.equal(tabs.tabs.length, 3);
});

test('launch boundary rejects malformed paths and selections outside their folder', () => {
	for (const value of [
		null,
		{},
		{ path: 'root/../etc' },
		{ path: 'root/a', select: 'root/b/file' },
		{ path: 'root/a', select: 'root/a/sub/file' },
		{ path: 'root/a\u0000' },
		{ path: '@remote/invalid' }
	])
		assert.equal(parseDesktopLaunch(value), null);
	assert.deepEqual(parseDesktopLaunch({ path: 'root/a', select: 'root/a/file.txt' }), {
		path: 'root/a',
		select: 'root/a/file.txt'
	});
});
