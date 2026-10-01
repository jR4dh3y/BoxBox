/**
 * System API module for system-level information
 */

import { api } from './client';

/**
 * System drive information (from df command)
 */
export interface SystemDrive {
	device: string;
	mountPoint: string;
	fsType?: string;
	totalBytes: number;
	usedBytes: number;
	freeBytes: number;
	usedPct: number;
}

/**
 * System drives response
 */
export interface SystemDrivesResponse {
	drives: SystemDrive[];
}

/** Upload settings chosen by the server operator */
export interface UploadConfig {
	chunkSizeBytes: number;
}

/**
 * Get the chunk size uploads should use
 * GET /api/v1/system/upload
 */
export async function getUploadConfig(): Promise<UploadConfig> {
	return api.get<UploadConfig>('/system/upload');
}

/**
 * Get all system drives/filesystems
 * GET /api/v1/system/drives
 */
export async function getSystemDrives(): Promise<SystemDrivesResponse> {
	return api.get<SystemDrivesResponse>('/system/drives');
}
