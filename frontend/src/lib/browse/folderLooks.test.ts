import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
	DEFAULT_LOOK,
	lookEmblem,
	parseFolderLooks,
	specialEmblem,
	withFolderLook
} from './folderLooks.js';

describe('folder looks', () => {
	test('a default look is stored as nothing', () => {
		const looks = withFolderLook({}, 'home/a', { ...DEFAULT_LOOK, color: '#ef4444' });
		assert.deepEqual(Object.keys(looks), ['home/a']);
		assert.deepEqual(withFolderLook(looks, 'home/a', DEFAULT_LOOK), {});
	});

	test('special folders are places and their namesakes just inside a drive', () => {
		const places = new Set(['downloads']);
		assert.equal(specialEmblem('downloads', places), 'download');
		assert.equal(specialEmblem('home/Downloads', places), 'download');
		assert.equal(specialEmblem('home/projects/Downloads', places), null);
		assert.equal(specialEmblem('music', places), null);
		assert.equal(specialEmblem('home/notes', places), null);
	});

	test('a look keeps the own emblem, hides it, or picks another', () => {
		assert.equal(lookEmblem(undefined, 'download'), 'download');
		assert.equal(lookEmblem({ ...DEFAULT_LOOK, emblem: { kind: 'none' } }, 'download'), null);
		assert.equal(
			lookEmblem({ ...DEFAULT_LOOK, emblem: { kind: 'icon', name: 'code' } }, null),
			'code'
		);
	});

	test('saved looks drop what does not parse', () => {
		assert.deepEqual(
			parseFolderLooks({
				a: { emblem: { kind: 'icon', name: 'code' }, color: '#22c55e', emblemColor: 'red' },
				b: { emblem: { kind: 'icon', name: '<svg>' } },
				c: 'x'
			}),
			{ a: { emblem: { kind: 'icon', name: 'code' }, color: '#22c55e', emblemColor: null } }
		);
	});
});
