import { test } from 'node:test';
import assert from 'node:assert/strict';
import { FileChooser, globMatch } from './chooser.svelte';
test('portal globs cover wildcards, case classes and literal regex characters', () => {
	assert.equal(globMatch('*.[pP][nN][gG]', 'image.PNG'), true);
	assert.equal(globMatch('Report?.txt', 'Report1.txt'), true);
	assert.equal(globMatch('data(1).txt', 'data(1).txt'), true);
	assert.equal(globMatch('*.txt', 'image.png'), false);
	assert.equal(globMatch('[!a]*', 'beta'), true);
});

test('MIME picker shows content-detected extensionless files and excludes other types', () => {
	const chooser = new FileChooser('request');
	chooser.request = {
		kind: 'open',
		title: '',
		appId: '',
		acceptLabel: '',
		multiple: false,
		directory: false,
		folder: 'home',
		name: '',
		files: [],
		filter: 0,
		filters: [{ name: 'Images', rules: [{ kind: 1, value: 'image/png' }] }],
		choices: []
	};
	const entry = {
		size: 8,
		isDir: false,
		permissions: '-rw-------',
		modTime: '2026-11-01T00:00:00Z'
	};
	chooser.items = [
		{ ...entry, name: 'logo', path: 'home/logo', mimeType: 'image/png' },
		{ ...entry, name: 'Report', path: 'home/Report', mimeType: 'text/plain; charset=utf-8' }
	];
	assert.deepEqual(
		chooser.visible.map((item) => item.name),
		['logo']
	);
});
