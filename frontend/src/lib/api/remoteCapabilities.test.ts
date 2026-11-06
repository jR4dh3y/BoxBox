import assert from 'node:assert/strict';
import { after, before, test } from 'node:test';
import { isNativePath, openDesktopFile, supportsServerTransfer } from './desktop';
import {
	listDesktopApps,
	openDesktopTerminal,
	openDesktopWithApp,
	setDesktopDefaultApp
} from './desktopApps';
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
	await assert.rejects(setDesktopDefaultApp(remote, 'app'));
	await assert.rejects(openDesktopTerminal(remote));
	await assert.rejects(trashDesktopFile(remote));
	await assert.rejects(setDesktopClipboard([remote], 'copy'));
	assert.equal(requests, 0);
});

test('remote selection and background menus expose server capabilities', () => {
	const file = { name: 'a.txt', path: remote, isDir: false, size: 1, modTime: '', permissions: '' };
	const actions = getFileContextMenuItems({ items: [file], canPaste: true }).flatMap((item) =>
		typeof item === 'string' ? [] : [item.id]
	);
	for (const action of ['download', 'share', 'delete'])
		assert.ok(actions.some((id) => id === action));
	for (const action of ['open-with', 'open-terminal', 'delete-permanently'])
		assert.ok(!actions.some((id) => id === action));
	const background = getFileContextMenuItems({ items: [], canPaste: true });
	assert.ok(!background.some((item) => typeof item !== 'string' && item.id === 'open-terminal'));
});

test('native paths omit and refuse server-only share and download, per selected path', () => {
	const entry = (path: string, isDir = false) => ({
		name: path.split('/').at(-1) ?? path,
		path,
		isDir,
		size: 1,
		modTime: '',
		permissions: ''
	});
	const ids = (items: ReturnType<typeof entry>[]) =>
		getFileContextMenuItems({ items, canPaste: true }).flatMap((item) =>
			typeof item === 'string' ? [] : [item.id]
		);
	const native = entry('home/deck.pptx');
	for (const items of [[native], [entry('home/Folder', true)], [native, entry('home/b.txt')]]) {
		const actions = ids(items);
		assert.ok(!actions.includes('share') && !actions.includes('download'), actions.join());
		assert.ok(actions.includes('properties') && actions.includes('delete'));
		assert.equal(supportsServerTransfer(items), false);
	}
	assert.ok(ids([native]).includes('open-with'));
	// A remote item next to a local one still cannot be downloaded through a server.
	assert.equal(supportsServerTransfer([native, entry(remote)]), false);
	assert.equal(supportsServerTransfer([entry(remote)]), true);
	assert.equal(supportsServerTransfer([]), false);
});

test('read-only menus disable mutations while keeping copy and inspection available', () => {
	const items = [
		{
			name: 'notes.txt',
			path: 'media/notes.txt',
			isDir: false,
			size: 1,
			modTime: '',
			permissions: ''
		}
	];
	for (const selection of [[], items]) {
		const menu = getFileContextMenuItems({
			items: selection,
			canPaste: true,
			canCreate: true,
			readOnly: true
		});
		for (const action of ['new-file', 'new-folder', 'cut', 'paste', 'rename', 'delete']) {
			const item = menu.find((item) => item.id === action);
			if (item) assert.equal(item.disabled, true, `${action} must not offer a read-only mutation`);
		}
		for (const action of ['copy', 'properties']) {
			const item = menu.find((item) => item.id === action);
			if (item) assert.notEqual(item.disabled, true);
		}
	}
});
