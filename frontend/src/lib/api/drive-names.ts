/**
 * Drive name API client - manages custom drive names
 */

import { api } from './client';

export interface DriveNameMapping {
	mountPoint: string;
	customName: string;
}

export interface DriveNamesResponse {
	mappings: DriveNameMapping[];
}

export interface DriveNamesRequest {
	mountPoint: string;
	customName: string;
}

/**
 * Get all custom drive names
 */
export async function getDriveNames(): Promise<DriveNamesResponse> {
	return api.get<DriveNamesResponse>('/settings/drive-names');
}

/**
 * Set a custom name for a mount point
 */
export async function setDriveName(request: DriveNamesRequest): Promise<void> {
	await api.put<unknown>('/settings/drive-names', request);
}

/**
 * Remove a custom name for a mount point
 */
export async function deleteDriveName(mountPoint: string): Promise<void> {
	await api.delete<unknown>(`/settings/drive-names/${encodeURIComponent(mountPoint)}`);
}
