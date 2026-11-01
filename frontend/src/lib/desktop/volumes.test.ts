import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import type { DesktopVolume, DesktopVolumesResponse } from '#lib/api/desktopVolumes.js';
import { VolumesController, type VolumesClient } from './volumes.svelte';

function volume(overrides: Partial<DesktopVolume> = {}): DesktopVolume {
	return {
		id: '0123456789abcdef0123456789abcdef',
		name: 'Photos',
		driveName: 'USB DISK 3.0',
		mounted: true,
		readOnly: false,
		canMount: false,
		canUnmount: true,
		canEject: true,
		...overrides
	};
}

function response(overrides: Partial<DesktopVolumesResponse> = {}): DesktopVolumesResponse {
	return { available: true, volumes: [volume()], ...overrides };
}

function client(
	list: VolumesClient['list'],
	act: VolumesClient['act'] = async () => response()
): VolumesClient {
	return { list, act };
}

describe('volumes controller', () => {
	test('replaces the list with the snapshot an action returns', async () => {
		const controller = new VolumesController(
			client(
				async () => response({ volumes: [volume({ mounted: false, canMount: true })] }),
				async () => response()
			)
		);
		await controller.load();
		const unmounted = controller.snapshot.value?.volumes[0];
		if (!unmounted) throw new Error('missing initial snapshot');

		assert.equal(await controller.act(unmounted, 'mount'), true);
		assert.equal(controller.success, 'Mounted Photos.');
		assert.equal(controller.error, null);
		assert.equal(controller.snapshot.value?.volumes[0].mounted, true);
	});

	test('shows the server failure and keeps the last real snapshot', async () => {
		const initial = response();
		const controller = new VolumesController(
			client(
				async () => initial,
				async () => {
					throw new Error('Could not unmount Photos: target is busy');
				}
			)
		);
		await controller.load();
		assert.equal(await controller.act(initial.volumes[0], 'unmount'), false);
		assert.equal(controller.error, 'Could not unmount Photos: target is busy');
		assert.equal(controller.success, null);
		assert.deepEqual(controller.snapshot.value, initial);
	});

	test('a refresh clears the last action result', async () => {
		const controller = new VolumesController(
			client(
				async () => response(),
				async () => {
					throw new Error('target is busy');
				}
			)
		);
		await controller.load();
		await controller.act(volume(), 'eject');
		await controller.load();
		assert.equal(controller.error, null);
	});
});
