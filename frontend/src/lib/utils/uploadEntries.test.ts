import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { entriesFromFiles, foldersFor } from './uploadEntries';

const file = (name: string) => new File(['x'], name);

describe('foldersFor', () => {
	test('lists each folder once, parents first', () => {
		const entries = [
			{ file: file('a.txt'), relativePath: 'photos/2024/a.txt' },
			{ file: file('b.txt'), relativePath: 'photos/2024/b.txt' },
			{ file: file('c.txt'), relativePath: 'photos/c.txt' },
			{ file: file('d.txt'), relativePath: 'd.txt' }
		];
		assert.deepEqual(foldersFor(entries), ['photos', 'photos/2024']);
	});
});

describe('entriesFromFiles', () => {
	test('uses the file name when there is no folder path', () => {
		assert.deepEqual(
			entriesFromFiles([file('a.txt')]).map((entry) => entry.relativePath),
			['a.txt']
		);
	});
});
