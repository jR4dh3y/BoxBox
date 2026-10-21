/**
 * Removable-volume API for the local desktop app.
 *
 * Volume IDs come from the backend's current GIO snapshot. Send them back
 * unchanged; device paths and mount URIs are never accepted from the client.
 */

import { api } from './client';

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

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function record(value: unknown, label: string): Record<string, unknown> {
	if (!isRecord(value)) {
		throw new Error(`Invalid ${label} response`);
	}
	return value;
}

function requiredString(value: unknown, label: string): string {
	if (typeof value !== 'string' || value.length === 0) {
		throw new Error(`Invalid ${label} response`);
	}
	return value;
}

function optionalString(value: unknown, label: string): string | undefined {
	if (value === undefined || value === null) return undefined;
	if (typeof value !== 'string') throw new Error(`Invalid ${label} response`);
	return value;
}

function requiredBoolean(value: unknown, label: string): boolean {
	if (typeof value !== 'boolean') throw new Error(`Invalid ${label} response`);
	return value;
}

function optionalBytes(value: unknown, label: string): number | undefined {
	if (value === undefined || value === null) return undefined;
	if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
		throw new Error(`Invalid ${label} response`);
	}
	return value;
}

function parseVolume(value: unknown): DesktopVolume {
	const volume = record(value, 'volume');
	return {
		id: requiredString(volume.id, 'volume id'),
		name: requiredString(volume.name, 'volume name'),
		driveName: requiredString(volume.driveName, 'drive name'),
		device: optionalString(volume.device, 'volume device'),
		uuid: optionalString(volume.uuid, 'volume UUID'),
		mountUri: optionalString(volume.mountUri, 'mount URI'),
		mountPath: optionalString(volume.mountPath, 'mount path'),
		browsePath: optionalString(volume.browsePath, 'browse path'),
		mounted: requiredBoolean(volume.mounted, 'mounted state'),
		readOnly: requiredBoolean(volume.readOnly, 'read-only state'),
		totalBytes: optionalBytes(volume.totalBytes, 'total size'),
		freeBytes: optionalBytes(volume.freeBytes, 'free size'),
		canMount: requiredBoolean(volume.canMount, 'mount capability'),
		canUnmount: requiredBoolean(volume.canUnmount, 'unmount capability'),
		canEject: requiredBoolean(volume.canEject, 'eject capability')
	};
}

/** Parse the backend response into the typed shape used by the panel. */
export function parseDesktopVolumesResponse(value: unknown): DesktopVolumesResponse {
	const response = record(value, 'volumes');
	if (typeof response.available !== 'boolean') {
		throw new Error('Invalid volumes response');
	}
	if (!Array.isArray(response.volumes)) {
		throw new Error('Invalid volumes response');
	}
	const reason = optionalString(response.reason, 'availability reason');
	const volumes = response.volumes.map(parseVolume);
	return {
		available: response.available,
		...(reason ? { reason } : {}),
		volumes
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
