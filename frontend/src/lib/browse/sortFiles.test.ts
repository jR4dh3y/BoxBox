import assert from 'node:assert/strict';
import { test } from 'node:test';
import type { FileInfo } from '#lib/api/files.js';
import { sortFiles, visibleByDefault } from './sortFiles';
import { suggestedDirectories } from './pathSuggestions';

function entry(
	name: string,
	{ isDir = false, size = 0, modTime = '2026-01-01T00:00:00Z' } = {}
): FileInfo {
	return { name, path: `media/${name}`, isDir, size, modTime, permissions: '-rw-r--r--' };
}

const names = (items: FileInfo[]) => items.map((item) => item.name);
const folder = [
	entry('file10.txt', { size: 10, modTime: '2026-06-01T00:00:00Z' }),
	entry('.hidden', { size: 1, modTime: '2026-07-01T00:00:00Z' }),
	entry('file2.txt', { size: 10, modTime: '2026-06-01T00:00:00Z' }),
	entry('old-folder', { isDir: true, modTime: '2026-01-01T00:00:00Z' }),
	entry('new-folder', { isDir: true, modTime: '2026-05-01T00:00:00Z' }),
	entry('.config', { isDir: true, modTime: '2026-02-01T00:00:00Z' })
];

test('default order puts folders first, newest first within each group, with natural name ties', () => {
	assert.deepEqual(names(visibleByDefault(folder, true)), [
		'new-folder',
		'.config',
		'old-folder',
		'.hidden',
		'file2.txt',
		'file10.txt'
	]);
	assert.deepEqual(names(visibleByDefault(folder, false)), [
		'new-folder',
		'old-folder',
		'file2.txt',
		'file10.txt'
	]);
});

test('folders lead all explicit sorts and directions, and hidden entries follow the same order', () => {
	for (const field of ['name', 'type', 'modTime', 'size'] satisfies Parameters<
		typeof sortFiles
	>[1][]) {
		for (const direction of ['asc', 'desc'] satisfies Parameters<typeof sortFiles>[2][]) {
			const result = sortFiles(folder, field, direction);
			assert.ok(result.slice(0, 3).every((item) => item.isDir));
			assert.ok(result.slice(3).every((item) => !item.isDir));
		}
	}
	assert.deepEqual(names(sortFiles(folder, 'name', 'asc')), [
		'.config',
		'new-folder',
		'old-folder',
		'.hidden',
		'file2.txt',
		'file10.txt'
	]);
	assert.deepEqual(names(sortFiles(folder, 'name', 'desc')), [
		'old-folder',
		'new-folder',
		'.config',
		'file10.txt',
		'file2.txt',
		'.hidden'
	]);
	assert.deepEqual(names(sortFiles(folder, 'modTime', 'asc')), [
		'old-folder',
		'.config',
		'new-folder',
		'file2.txt',
		'file10.txt',
		'.hidden'
	]);
	assert.deepEqual(names(sortFiles(folder, 'size', 'desc')).slice(3), [
		'file2.txt',
		'file10.txt',
		'.hidden'
	]);
});

test('path suggestions use newest folders first, including hidden folders only when requested', () => {
	assert.deepEqual(names(suggestedDirectories(folder, '', false)), ['new-folder', 'old-folder']);
	assert.deepEqual(names(suggestedDirectories(folder, '.', false)), [
		'new-folder',
		'.config',
		'old-folder'
	]);
});

test('equal names in recursive searches use their full path as a deterministic tie-break', () => {
	const items = [
		{ ...entry('same.txt'), path: '@remote/id/media/folder10/same.txt' },
		{ ...entry('same.txt'), path: '@remote/id/media/folder2/same.txt' }
	];
	assert.deepEqual(
		sortFiles(items, 'modTime', 'desc').map((item) => item.path),
		[items[1].path, items[0].path]
	);
});

test('missing timestamps sort last and equal numeric names remain deterministic', () => {
	const items = [entry('file2.txt'), entry('file02.txt'), entry('unknown', { modTime: 'invalid' })];
	assert.deepEqual(names(visibleByDefault(items, true)), ['file02.txt', 'file2.txt', 'unknown']);
	assert.deepEqual(names(visibleByDefault([...items].reverse(), true)), [
		'file02.txt',
		'file2.txt',
		'unknown'
	]);
});

test('sorting leaves the cached listing untouched', () => {
	const before = [...folder];
	visibleByDefault(folder, true);
	assert.deepEqual(folder, before);
});
