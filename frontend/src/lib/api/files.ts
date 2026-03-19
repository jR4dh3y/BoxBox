/**
 * File API module for file operations
 * Requirements: 1.1, 8.1, 8.2, 8.3, 9.1
 */

import { api, getAccessToken } from './client';

/**
 * File/directory metadata
 */
export interface FileInfo {
	name: string;
	path: string;
	size: number;
	isDir: boolean;
	modTime: string;
	permissions: string;
	mimeType?: string;
}

/**
 * Paginated file list response
 */
export interface FileList {
	path: string;
	items: FileInfo[];
	totalCount: number;
	page: number;
	pageSize: number;
}

/**
 * Mount point information
 */
export interface MountPoint {
	name: string;
	readOnly: boolean;
	autoDiscover?: boolean;
}

/**
 * Mount points response
 */
export interface RootsResponse {
	roots: MountPoint[];
}

/**
 * Drive statistics
 */
export interface DriveStats {
	name: string;
	path: string;
	device?: string; // The underlying device (e.g., /dev/sda1)
	fsType?: string; // Filesystem type (e.g., ext4, ntfs)
	mountPoint?: string; // Actual mount point in the system
	totalBytes: number;
	freeBytes: number;
	usedBytes: number;
	usedPct: number;
	readOnly: boolean;
}

/**
 * Drive stats response
 */
export interface DriveStatsResponse {
	drives: DriveStats[];
}

/**
 * Options for listing directory contents
 */
export interface ListOptions {
	page?: number;
	pageSize?: number;
	sortBy?: 'name' | 'size' | 'modTime' | 'type';
	sortDir?: 'asc' | 'desc';
	filter?: string;
}

/**
 * Search results response
 */
export interface SearchResponse {
	path: string;
	query: string;
	results: FileInfo[];
	count: number;
}

/**
 * Create directory request
 */
interface CreateDirRequest {
	name: string;
}

/**
 * Rename request
 */
interface RenameRequest {
	newPath: string;
}

/**
 * Success message response
 */
interface MessageResponse {
	message: string;
}

/**
 * List all configured mount points (roots)
 * GET /api/v1/files
 */
export async function listRoots(): Promise<RootsResponse> {
	return api.get<RootsResponse>('/files');
}

/**
 * Get drive statistics for all mount points
 * GET /api/v1/files/stats
 */
export async function getDriveStats(): Promise<DriveStatsResponse> {
	return api.get<DriveStatsResponse>('/files/stats');
}

/**
 * List directory contents or get file info
 * GET /api/v1/files/*path
 */
export async function getPath(path: string, options?: ListOptions): Promise<FileList | FileInfo> {
	const params: Record<string, string | number | boolean | undefined> = {};

	if (options) {
		if (options.page !== undefined) params.page = options.page;
		if (options.pageSize !== undefined) params.pageSize = options.pageSize;
		if (options.sortBy) params.sortBy = options.sortBy;
		if (options.sortDir) params.sortDir = options.sortDir;
		if (options.filter) params.filter = options.filter;
	}

	return api.get<FileList | FileInfo>(`/files/${path}`, params);
}

/**
 * List directory contents with pagination
 * Returns FileList for directories
 */
export async function listDirectory(path: string, options?: ListOptions): Promise<FileList> {
	return getPath(path, options) as Promise<FileList>;
}

/**
 * Get file or directory info
 * Returns FileInfo
 */
export async function getFileInfo(path: string): Promise<FileInfo> {
	return getPath(path) as Promise<FileInfo>;
}

/**
 * Create a new directory
 * POST /api/v1/files/*path
 */
export async function createDirectory(basePath: string, name: string): Promise<FileInfo> {
	const body: CreateDirRequest = { name };
	return api.post<FileInfo>(`/files/${basePath}`, body);
}

/**
 * Rename or move a file/directory
 * PUT /api/v1/files/*path
 */
export async function rename(oldPath: string, newPath: string): Promise<FileInfo> {
	const body: RenameRequest = { newPath };
	return api.put<FileInfo>(`/files/${oldPath}`, body);
}

/**
 * Delete a file or directory
 * DELETE /api/v1/files/*path
 * @param confirm - Set to true to confirm directory deletion
 */
export async function deleteFile(path: string, confirm: boolean = false): Promise<MessageResponse> {
	const params = confirm ? { confirm: 'true' } : undefined;
	return api.delete<MessageResponse>(`/files/${path}`, params);
}

/**
 * Search for files by name
 * GET /api/v1/search?path=&q=
 */
export async function search(path: string, query: string): Promise<SearchResponse> {
	return api.get<SearchResponse>('/search', { path, q: query });
}

/**
 * Get the preview URL for a file (for streaming media, images, etc.)
 * This URL can be used directly in <video>, <audio>, <img>, <iframe> src
 */
export function getPreviewUrl(path: string): string {
	const token = getAccessToken();
	// Don't double-encode the path - just encode special characters
	const encodedPath = path
		.split('/')
		.map((segment) => encodeURIComponent(segment))
		.join('/');
	const baseUrl = `/api/v1/stream/preview/${encodedPath}`;
	return token ? `${baseUrl}?token=${encodeURIComponent(token)}` : baseUrl;
}

/**
 * Get the download URL for a file
 */
export function getDownloadUrl(path: string): string {
	const token = getAccessToken();
	const encodedPath = path
		.split('/')
		.map((segment) => encodeURIComponent(segment))
		.join('/');
	const baseUrl = `/api/v1/stream/download/${encodedPath}`;
	return token ? `${baseUrl}?token=${encodeURIComponent(token)}` : baseUrl;
}

/**
 * Fetch file content as text (for code/text preview)
 */
export async function getFileContent(path: string): Promise<string> {
	// If path is already a full URL (from getPreviewUrl), use it directly
	const url = path.startsWith('/api/') || path.startsWith('http') ? path : getPreviewUrl(path);
	const response = await fetch(url);
	if (!response.ok) {
		throw new Error(`Failed to fetch file: ${response.statusText}`);
	}
	return response.text();
}
