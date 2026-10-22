/**
 * Removable-volume API for the local desktop app.
 *
 * Volume IDs come from the backend's current GIO snapshot. Send them back
 * unchanged; device paths and mount URIs are never accepted from the client.
 */

import { api } from './client';
import { array, boolean, nonEmptyString, optionalByteCount, optionalString, record } from './parse';

export interface DesktopVolume {
	id: string;
	name: string;
	driveName: string;
	device?: string;
	uuid?: string;
	mountUri?: string;
	mountPath?: string;
	browsePath?: string;
	mounted: boolean;
	readOnly: boolean;
	totalBytes?: number;
	freeBytes?: number;
	canMount: boolean;
	canUnmount: boolean;
	canEject: boolean;
}

export interface DesktopVolumesResponse {
	available: boolean;
	reason?: string;
	volumes: DesktopVolume[];
}

export type DesktopVolumeAction = 'mount' | 'unmount' | 'eject';

const DESKTOP_VOLUMES_ENDPOINT = '/desktop/volumes';

function parseVolume(value: unknown): DesktopVolume {
	const volume = record(value, 'volume');
	return {
		id: nonEmptyString(volume.id, 'volume id'),
		name: nonEmptyString(volume.name, 'volume name'),
		driveName: nonEmptyString(volume.driveName, 'drive name'),
		device: optionalString(volume.device, 'volume device'),
		uuid: optionalString(volume.uuid, 'volume UUID'),
		mountUri: optionalString(volume.mountUri, 'mount URI'),
		mountPath: optionalString(volume.mountPath, 'mount path'),
		browsePath: optionalString(volume.browsePath, 'browse path'),
		mounted: boolean(volume.mounted, 'mounted state'),
		readOnly: boolean(volume.readOnly, 'read-only state'),
		totalBytes: optionalByteCount(volume.totalBytes, 'total size'),
		freeBytes: optionalByteCount(volume.freeBytes, 'free size'),
		canMount: boolean(volume.canMount, 'mount capability'),
		canUnmount: boolean(volume.canUnmount, 'unmount capability'),
		canEject: boolean(volume.canEject, 'eject capability')
	};
}

/** Parse the backend response into the typed shape used by the panel. */
export function parseDesktopVolumesResponse(value: unknown): DesktopVolumesResponse {
	const response = record(value, 'volumes response');
	const reason = optionalString(response.reason, 'availability reason');
	return {
		available: boolean(response.available, 'volumes response'),
		...(reason ? { reason } : {}),
		volumes: array(response.volumes, 'volumes response').map(parseVolume)
	};
}

/** List mounted and mountable removable volumes. */
export async function listDesktopVolumes(signal?: AbortSignal): Promise<DesktopVolumesResponse> {
	const response = await api.get<unknown>(DESKTOP_VOLUMES_ENDPOINT, undefined, signal);
	return parseDesktopVolumesResponse(response);
}

/** Mount, unmount or eject one volume and return the refreshed volume list. */
export async function runDesktopVolumeAction(
	volumeId: string,
	action: DesktopVolumeAction
): Promise<DesktopVolumesResponse> {
	const response = await api.post<unknown>(
		`${DESKTOP_VOLUMES_ENDPOINT}/${encodeURIComponent(volumeId)}/${action}`
	);
	return parseDesktopVolumesResponse(response);
}
