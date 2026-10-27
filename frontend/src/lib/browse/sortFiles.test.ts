import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { FileInfo } from '#lib/api/files.js';
import { sortFiles } from './sortFiles';
import { suggestedDirectories } from './pathSuggestions';

function entry(
	name: string,
	{ isDir = false, size = 0, modTime = '2026-01-01T00:00:00Z' } = {}
): FileInfo {
	return { name, path: `media/${name}`, isDir, size, modTime, permissions: '-rw-r--r--' };
}

const names = (items: FileInfo[]) => items.map((item) => item.name);

const folder = [
	entry('b.txt', { size: 3, modTime: '2026-03-01T00:00:00Z' }),
	entry('B.txt', { size: 1, modTime: '2026-01-01T00:00:00Z' }),
	entry('10.txt', { size: 2, modTime: '2026-02-01T00:00:00Z' }),
	entry('9.txt', { size: 2, modTime: '2026-04-01T00:00:00Z' }),
	entry('.hidden', { size: 9 }),
	entry('Zed', { isDir: true }),
	entry('apps', { isDir: true }),
	entry('.config', { isDir: true })
];

test('names sort naturally, case only breaks ties, and folders come first', () => {
	assert.deepEqual(names(sortFiles(folder, 'name', 'asc')), [
		'apps',
		'Zed',
		'9.txt',
		'10.txt',
		'b.txt',
		'B.txt',
		'.config',
		'.hidden'
	]);
});

test('hidden entries stay last and folders stay first in both directions', () => {
	const descending = names(sortFiles(folder, 'name', 'desc'));
	assert.deepEqual(descending.slice(-2), ['.config', '.hidden']);
	assert.deepEqual(descending.slice(0, 2), ['Zed', 'apps']);
	assert.equal(descending[2], 'B.txt');
});

test('type sort puts folders first in both directions', () => {
	assert.deepEqual(names(sortFiles(folder, 'type', 'asc')).slice(0, 2), ['apps', 'Zed']);
	assert.deepEqual(names(sortFiles(folder, 'type', 'desc')), [
		'Zed',
		'apps',
		'B.txt',
		'b.txt',
		'10.txt',
		'9.txt',
		'.config',
		'.hidden'
	]);
});

test('size and date sort by value, with the name breaking ties', () => {
	assert.deepEqual(names(sortFiles(folder, 'size', 'asc')).slice(0, 6), [
		'apps',
		'Zed',
		'B.txt',
		'9.txt',
		'10.txt',
		'b.txt'
	]);
	assert.deepEqual(names(sortFiles(folder, 'modTime', 'desc')).slice(0, 4), [
		'9.txt',
		'b.txt',
		'10.txt',
		'Zed'
	]);
});

test('sorting returns a copy and leaves the cached listing untouched', () => {
	const before = names(folder);
	sortFiles(folder, 'size', 'desc');
	assert.deepEqual(names(folder), before);
});

test('path suggestions offer folders by name, and hidden ones only when asked', () => {
	assert.deepEqual(names(suggestedDirectories(folder, '', false)), ['apps', 'Zed']);
	assert.deepEqual(names(suggestedDirectories(folder, '.', false)), ['apps', 'Zed', '.config']);
	assert.deepEqual(names(suggestedDirectories(folder, '', true)), ['apps', 'Zed', '.config']);
});
