import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
	parseDesktopVolumesResponse,
	type DesktopVolume,
	type DesktopVolumesResponse
} from './desktopVolumes';

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
			/Invalid volume name/
		);
		assert.throws(
			() =>
				parseDesktopVolumesResponse({
					available: true,
					volumes: [{ ...volume(), mounted: 'yes' }]
				}),
			/Invalid mounted state/
		);
		assert.throws(
			() =>
				parseDesktopVolumesResponse({
					available: true,
					volumes: [{ ...volume(), freeBytes: -1 }]
				}),
			/Invalid free size/
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
