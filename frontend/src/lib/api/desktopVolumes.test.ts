import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
	parseDesktopVolumesResponse,
	type DesktopVolume,
	type DesktopVolumesResponse
} from './desktopVolumes';
import {
	DesktopVolumesController,
	type DesktopVolumesClient
} from './desktopVolumesController.svelte';

function volume(overrides: Partial<DesktopVolume> = {}): DesktopVolume {
	return {
		id: '0123456789abcdef0123456789abcdef',
		name: 'Photos',
		driveName: 'USB DISK 3.0',
		device: '/dev/sdb1',
		uuid: '1234-ABCD',
		mountUri: 'file:///run/media/radhey/Photos',
		mountPath: '/run/media/radhey/Photos',
		browsePath: 'root/run/media/radhey/Photos',
		mounted: true,
		readOnly: false,
		totalBytes: 64 * 1024 * 1024 * 1024,
		freeBytes: 16 * 1024 * 1024 * 1024,
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
	list: DesktopVolumesClient['list'],
	act: DesktopVolumesClient['act'] = async () => response()
): DesktopVolumesClient {
	return { list, act };
}

describe('desktop volume response parsing', () => {
	test('keeps the typed snapshot and optional reason', () => {
		const parsed = parseDesktopVolumesResponse({
			available: false,
			reason: 'GIO is not installed.',
			volumes: []
		});
		assert.deepEqual(parsed, { available: false, reason: 'GIO is not installed.', volumes: [] });
		assert.deepEqual(parseDesktopVolumesResponse(response()), response());
	});

	test('parses the unavailable handler body without masking its reason', () => {
		const body: unknown = JSON.parse(
			'{"available":false,"reason":"The gio command is not installed.","volumes":[]}'
		);
		const parsed = parseDesktopVolumesResponse(body);
		assert.equal(parsed.available, false);
		assert.equal(parsed.reason, 'The gio command is not installed.');
		assert.deepEqual(parsed.volumes, []);
	});

	test('rejects malformed volume data instead of trusting JSON', () => {
		const missingName: Record<string, unknown> = { ...volume() };
		delete missingName.name;
		assert.throws(
			() => parseDesktopVolumesResponse({ available: true, volumes: [missingName] }),
			/Invalid volume name response/
		);
		assert.throws(
			() =>
				parseDesktopVolumesResponse({
					available: true,
					volumes: [{ ...volume(), mounted: 'yes' }]
				}),
			/Invalid mounted state response/
		);
		assert.throws(
			() =>
				parseDesktopVolumesResponse({
					available: true,
					volumes: [{ ...volume(), freeBytes: -1 }]
				}),
			/Invalid free size response/
		);
		assert.throws(
			() => parseDesktopVolumesResponse({ available: true, volumes: {} }),
			/Invalid volumes response/
		);
		assert.throws(
			() =>
				parseDesktopVolumesResponse({
					available: false,
					reason: 'The gio command is not installed.',
					volumes: null
				}),
			/Invalid volumes response/
		);
	});
});

describe('desktop volumes controller', () => {
	test('loads and replaces the list after a successful action', async () => {
		const controller = new DesktopVolumesController(
			client(
				async () => response({ volumes: [volume({ mounted: false, canMount: true })] }),
				async () => response()
			)
		);
		await controller.load();
		assert.equal(controller.loadState.kind, 'ready');
		assert.equal(controller.refreshing, false);

		const initial = controller.loadState;
		if (initial.kind !== 'ready') throw new Error('missing initial response');
		const changed = await controller.act(initial.response.volumes[0], 'mount');
		assert.equal(changed, true);
		assert.deepEqual(controller.notice, { kind: 'success', message: 'Mounted Photos.' });
		assert.equal(controller.actionState.kind, 'idle');
		if (controller.loadState.kind !== 'ready') throw new Error('missing refreshed response');
		assert.equal(controller.loadState.response.volumes[0].mounted, true);
	});

	test('shows the server failure and keeps the last real snapshot', async () => {
		const initial = response();
		const controller = new DesktopVolumesController(
			client(
				async () => initial,
				async () => {
					throw new Error('Could not unmount Photos: target is busy');
				}
			)
		);
		await controller.load();
		const changed = await controller.act(initial.volumes[0], 'unmount');
		assert.equal(changed, false);
		assert.deepEqual(controller.notice, {
			kind: 'error',
			message: 'Could not unmount Photos: target is busy'
		});
		assert.equal(controller.actionState.kind, 'failed');
		if (controller.loadState.kind !== 'ready') throw new Error('missing snapshot');
		assert.deepEqual(controller.loadState.response, initial);
	});

	test('keeps existing data when a refresh fails', async () => {
		let calls = 0;
		const controller = new DesktopVolumesController(
			client(async () => {
				calls += 1;
				if (calls === 1) return response();
				throw new Error('The system volume service is unavailable');
			})
		);
		await controller.load();
		await controller.load();
		if (controller.loadState.kind !== 'ready') throw new Error('existing data was dropped');
		assert.deepEqual(controller.notice, {
			kind: 'error',
			message: 'The system volume service is unavailable'
		});
		assert.equal(controller.refreshing, false);
	});

	test('ignores an older list response that arrives last', async () => {
		let resolveFirst: (value: DesktopVolumesResponse) => void = () => {
			throw new Error('first request did not start');
		};
		let resolveSecond: (value: DesktopVolumesResponse) => void = () => {
			throw new Error('second request did not start');
		};
		let requests = 0;
		const controller = new DesktopVolumesController(
			client(
				() =>
					new Promise<DesktopVolumesResponse>((resolve) => {
						requests += 1;
						if (requests === 1) resolveFirst = resolve;
						else resolveSecond = resolve;
					})
			)
		);
		const firstLoad = controller.load();
		const secondLoad = controller.load();
		const newest = response({ volumes: [volume({ name: 'Newest' })] });
		resolveSecond(newest);
		await secondLoad;
		resolveFirst(response({ volumes: [volume({ name: 'Stale' })] }));
		await firstLoad;
		if (controller.loadState.kind !== 'ready') throw new Error('missing newest response');
		assert.equal(controller.loadState.response.volumes[0].name, 'Newest');
	});

	test('an initial failure becomes a retryable failed state', async () => {
		const controller = new DesktopVolumesController(
			client(async () => {
				throw new Error('gio not available');
			})
		);
		await controller.load();
		assert.deepEqual(controller.loadState, { kind: 'failed', message: 'gio not available' });
	});
});
