import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mountedVolumeDrives } from './volumeCatalog';
import type { DesktopVolume } from '#lib/api/desktopVolumes.js';

const device: DesktopVolume = {
	id: 'usb',
	name: 'Photos',
	driveName: 'USB drive',
	mounted: true,
	readOnly: true,
	browsePath: 'root/media/photos',
	totalBytes: 100,
	freeBytes: 60,
	canMount: false,
	canUnmount: true,
	canEject: true
};
test('only mounted, browsable devices become This PC cards and retain their name and access', () => {
	const drives = mountedVolumeDrives([
		device,
		{ ...device, id: 'unmounted', mounted: false },
		{ ...device, id: 'unbrowsable', browsePath: undefined }
	]);
	assert.equal(drives.length, 1);
	assert.equal(drives[0].name, device.browsePath);
	assert.equal(drives[0].label, 'Photos');
	assert.equal(drives[0].readOnly, true);
	assert.equal(drives[0].usedPct, 40);
});
