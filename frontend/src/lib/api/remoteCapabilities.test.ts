import assert from 'node:assert/strict';
import { after, before, test } from 'node:test';
import { isNativePath, openDesktopFile } from './desktop';
import { listDesktopApps, openDesktopTerminal, openDesktopWithApp } from './desktopApps';
import { setDesktopClipboard } from './desktopClipboard';
import { trashDesktopFile } from './desktopTrash';
import { getFileContextMenuItems } from '../utils/fileContextMenu';

const remote = '@remote/12345678-1234-4234-8234-123456789abc/media/a.txt';
const documentBefore = globalThis.document;
const fetchBefore = globalThis.fetch;
let requests = 0;
before(() =>
	Object.assign(globalThis, {
		document: { querySelector: () => ({}) },
		fetch: () => {
			requests++;
			throw new Error('Native request escaped');
		}
	})
);
after(() => Object.assign(globalThis, { document: documentBefore, fetch: fetchBefore }));

test('remote paths never reach native open, applications, terminal, trash or clipboard endpoints', async () => {
	assert.equal(isNativePath('home/a.txt'), true);
	assert.equal(isNativePath(remote), false);
	await assert.rejects(openDesktopFile(remote));
	await assert.rejects(listDesktopApps(remote));
	await assert.rejects(openDesktopWithApp(remote, 'app'));
	await assert.rejects(openDesktopTerminal(remote));
	await assert.rejects(trashDesktopFile(remote));
	await assert.rejects(setDesktopClipboard([remote], 'copy'));
	assert.equal(requests, 0);
});

test('remote selection and background menus expose server capabilities', () => {
	const file = { name: 'a.txt', path: remote, isDir: false, size: 1, modTime: '', permissions: '' };
	const actions = getFileContextMenuItems({ items: [file], canPaste: true, path: remote }).flatMap(
		(item) => (typeof item === 'string' ? [] : [item.id])
	);
	for (const action of ['download', 'share', 'delete'])
		assert.ok(actions.some((id) => id === action));
	for (const action of ['open-with', 'open-terminal', 'delete-permanently'])
		assert.ok(!actions.some((id) => id === action));
	const background = getFileContextMenuItems({ items: [], canPaste: true, path: remote });
	assert.ok(!background.some((item) => typeof item !== 'string' && item.id === 'open-terminal'));
});
