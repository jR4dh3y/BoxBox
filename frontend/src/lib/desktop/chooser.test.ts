import { test } from 'node:test';
import assert from 'node:assert/strict';
import { FileChooser, globMatch } from './chooser.svelte';
import { VolumesController } from './volumes.svelte';
import type { DesktopVolume } from '#lib/api/desktopVolumes.js';
test('portal globs cover wildcards, case classes and literal regex characters', () => {
	assert.equal(globMatch('*.[pP][nN][gG]', 'image.PNG'), true);
	assert.equal(globMatch('Report?.txt', 'Report1.txt'), true);
	assert.equal(globMatch('data(1).txt', 'data(1).txt'), true);
	assert.equal(globMatch('*.txt', 'image.png'), false);
	assert.equal(globMatch('[!a]*', 'beta'), true);
});

function imageChooser() {
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
	return chooser;
}

test('MIME picker shows content-detected extensionless files and excludes other types', () => {
	assert.deepEqual(
		imageChooser().visible.map((item) => item.name),
		['logo']
	);
});

test('folder search matches names without case and composes with MIME, hidden and directory filters', () => {
	const chooser = imageChooser();
	const entry = chooser.items[0];
	assert.ok(entry);
	chooser.items.push(
		{ ...entry, name: 'Logo2.png', path: 'home/Logo2.png', modTime: '2026-11-02T00:00:00Z' },
		{ ...entry, name: 'Logos', path: 'home/Logos', isDir: true },
		{ ...entry, name: '.logo.png', path: 'home/.logo.png' },
		{ ...entry, name: 'logo.txt', path: 'home/logo.txt', mimeType: 'text/plain' }
	);
	chooser.search('  LOGO  ');
	assert.deepEqual(
		chooser.visible.map((item) => item.name),
		['Logos', 'Logo2.png', 'logo']
	);
	chooser.hidden = true;
	assert.ok(chooser.visible.some((item) => item.name === '.logo.png'));
	assert.ok(chooser.request);
	chooser.request.directory = true;
	assert.deepEqual(
		chooser.visible.map((item) => item.name),
		['Logos']
	);
	chooser.search('no-such-file');
	assert.equal(chooser.visible.length, 0);
	chooser.search('');
	assert.deepEqual(
		chooser.visible.map((item) => item.name),
		['Logos']
	);
});

test('search removes hidden selections and clearing search does not select them again', () => {
	const chooser = imageChooser();
	chooser.select(new Set(['home/logo']));
	chooser.search('LOG');
	assert.deepEqual([...chooser.selected], ['home/logo']);
	chooser.search('absent');
	assert.equal(chooser.selected.size, 0);
	chooser.select(new Set(['home/logo']));
	assert.equal(chooser.selected.size, 0);
	chooser.search('');
	assert.equal(chooser.selected.size, 0);
	assert.equal(chooser.visible.length, 1);
});

test('searching a save destination keeps the entered filename', () => {
	const chooser = imageChooser();
	assert.ok(chooser.request);
	chooser.request.kind = 'save';
	chooser.name = 'holiday.png';
	chooser.search('logo');
	assert.equal(chooser.name, 'holiday.png');
});

test('Other locations clears file selection and cannot submit the previous destination', async () => {
	const chooser = imageChooser();
	chooser.select(new Set(['home/logo']));
	chooser.search('logo');
	chooser.showLocations();
	assert.equal(chooser.view, 'locations');
	assert.equal(chooser.selected.size, 0);
	assert.equal(chooser.query, '');
	await chooser.accept();
	assert.equal(chooser.busy, false);
	assert.equal(chooser.done, false);
	assert.equal(chooser.error, '');
});

const removable: DesktopVolume = {
	id: 'usb',
	name: 'Photos',
	driveName: 'USB drive',
	mounted: false,
	readOnly: false,
	canMount: true,
	canUnmount: false,
	canEject: false
};

test('mounting in Other locations opens the fresh backend browse path', async () => {
	const actions: string[] = [];
	const volumes = new VolumesController({
		list: async () => ({ available: true, volumes: [removable] }),
		act: async (id, action) => {
			actions.push(`${id}/${action}`);
			return {
				available: true,
				volumes: [{ ...removable, mounted: true, browsePath: 'root/media/Photos' }]
			};
		}
	});
	const chooser = new FileChooser('request', volumes);
	chooser.showLocations();
	const opened: string[] = [];
	chooser.open = async (path) => {
		opened.push(path);
	};
	await chooser.openVolume(removable);
	assert.deepEqual(actions, ['usb/mount']);
	assert.deepEqual(opened, ['root/media/Photos']);
});

test('failed mounts stay in Other locations and expose the action error', async () => {
	const volumes = new VolumesController({
		list: async () => ({ available: true, volumes: [removable] }),
		act: async () => {
			throw new Error('Mount permission denied');
		}
	});
	const chooser = new FileChooser('request', volumes);
	chooser.showLocations();
	await chooser.openVolume(removable);
	assert.equal(chooser.view, 'locations');
	assert.equal(volumes.error, 'Mount permission denied');
	await volumes.snapshot.load();
	assert.equal(volumes.error, 'Mount permission denied');
	assert.equal(chooser.path, '');
});

test('remote saves and folder grants cannot submit a non-local destination', async () => {
	const chooser = imageChooser();
	chooser.path = '@remote/11111111-1111-4111-8111-111111111111/photos';
	assert.equal(chooser.remoteUnavailable, false);
	assert.ok(chooser.request);
	chooser.request.directory = true;
	assert.equal(chooser.remoteUnavailable, true);
	await chooser.accept();
	assert.equal(chooser.busy, false);
	assert.equal(chooser.done, false);
	chooser.request.directory = false;
	chooser.request.kind = 'save';
	assert.equal(chooser.remoteUnavailable, true);
	await chooser.accept();
	assert.equal(chooser.done, false);
});

test('up from a remote drive returns to Other locations', () => {
	const chooser = imageChooser();
	chooser.path = '@remote/11111111-1111-4111-8111-111111111111/photos';
	chooser.parent();
	assert.equal(chooser.view, 'locations');
	assert.equal(chooser.selected.size, 0);
});
