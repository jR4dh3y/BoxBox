import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
	folderView,
	isRootPath,
	parseFolderViews,
	rememberFolderMode,
	rememberFolderSort
} from './folderViews.js';

const defaults = {
	mode: 'list',
	sort: { by: 'modTime', dir: 'desc' },
	perFolder: true
} as const;

describe('folder views', () => {
	test('a folder never set opens with the defaults', () => {
		assert.deepEqual(folderView({}, 'media', defaults), { mode: 'list', sort: defaults.sort });
	});

	test('a folder keeps its own view and sort', () => {
		let views = rememberFolderMode({}, 'media', 'grid', 'list');
		views = rememberFolderSort(views, 'media', { by: 'name', dir: 'asc' }, defaults.sort);
		assert.deepEqual(folderView(views, 'media', defaults), {
			mode: 'grid',
			sort: { by: 'name', dir: 'asc' }
		});
		assert.deepEqual(folderView(views, 'media/sub', defaults).mode, 'list');
	});

	test('picking the default stores nothing', () => {
		let views = rememberFolderMode({}, 'media', 'grid', 'list');
		views = rememberFolderMode(views, 'media', 'list', 'list');
		assert.deepEqual(views, {});
		views = rememberFolderSort(views, 'media', { by: 'name', dir: 'asc' }, defaults.sort);
		views = rememberFolderSort(views, 'media', defaults.sort, defaults.sort);
		assert.deepEqual(views, {});
	});

	test('with views not kept per folder, the sort is still the folder’s', () => {
		const views = { media: { mode: 'grid', sort: { by: 'size', dir: 'asc' } } } as const;
		assert.deepEqual(folderView(views, 'media', { ...defaults, perFolder: false }), {
			mode: 'list',
			sort: { by: 'size', dir: 'asc' }
		});
	});

	test('the root of this server and of a paired server is not a folder', () => {
		assert.equal(isRootPath(''), true);
		assert.equal(isRootPath('@remote/aaaabbbb-cccc-4ddd-8eee-ffffffffffff'), true);
		assert.equal(isRootPath('@remote/aaaabbbb-cccc-4ddd-8eee-ffffffffffff/media'), false);
		assert.equal(isRootPath('media'), false);
	});

	test('saved views drop entries that no longer parse', () => {
		assert.deepEqual(
			parseFolderViews({
				a: { mode: 'grid', sort: { by: 'name', dir: 'up' } },
				b: { mode: 'tiles' },
				c: 'grid',
				d: { sort: { by: 'size', dir: 'asc' } }
			}),
			{ a: { mode: 'grid' }, d: { sort: { by: 'size', dir: 'asc' } } }
		);
		assert.deepEqual(parseFolderViews(null), {});
	});
});
