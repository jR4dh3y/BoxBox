/**
 * File API module for file operations
 */

import { api, buildUrl } from './client';
import { parseDirectory } from './directory';
import { tokenStorage } from '#lib/utils/storage.js';

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

/** Every entry of one directory, hidden ones included, in no particular order. */
export interface FileList {
	path: string;
	items: FileInfo[];
}

/**
 * Mount point information
 */
export interface MountPoint {
	name: string;
	path: string;
	readOnly: boolean;
	autoDiscover?: boolean;
	/** Drives are storage roots; places are folder shortcuts such as Downloads. */
	kind?: 'drive' | 'place';
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
 * Search results response
 */
export interface SearchResponse {
	path: string;
	query: string;
	results: FileInfo[];
	count: number;
}

/**
 * Create file/directory request
 */
interface CreateItemRequest {
	name: string;
	type?: 'file' | 'directory';
	content?: string;
}

/**
 * Rename request
 */
interface RenameRequest {
	newPath: string;
}

/**
 * Save file content request
 */
interface SaveFileRequest {
	content: string;
}

/**
 * Success message response
 */
interface MessageResponse {
	message: string;
}

function encodeRoutePath(path: string): string {
	return path.split('/').map(encodeURIComponent).join('/');
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

function listEndpoint(path: string): string {
	return `/files/list/${encodeRoutePath(path)}`;
}

/** Every entry of a directory in one response. The client sorts and filters. */
export async function listDirectory(path: string, signal?: AbortSignal): Promise<FileList> {
	return parseDirectory(await api.get<unknown>(listEndpoint(path), undefined, signal));
}

/** The URL `listDirectory` requests. static/early-fetch.js must start the same one. */
export function directoryUrl(path: string): string {
	return buildUrl(listEndpoint(path));
}

/**
 * Get file or directory info
 * Returns FileInfo
 */
export async function getFileInfo(path: string): Promise<FileInfo> {
	return api.get<FileInfo>(`/files/${encodeRoutePath(path)}`);
}

/**
 * Create a new directory
 * POST /api/v1/files/*path
 */
export async function createDirectory(basePath: string, name: string): Promise<FileInfo> {
	const body: CreateItemRequest = { name, type: 'directory' };
	return api.post<FileInfo>(`/files/${encodeRoutePath(basePath)}`, body);
}

/**
 * Create a new empty file
 * POST /api/v1/files/*path
 */
export async function createFile(
	basePath: string,
	name: string,
	content: string = ''
): Promise<FileInfo> {
	const body: CreateItemRequest = { name, type: 'file', content };
	return api.post<FileInfo>(`/files/${encodeRoutePath(basePath)}`, body);
}

/**
 * Rename or move a file/directory
 * PUT /api/v1/files/*path
 */
export async function rename(oldPath: string, newPath: string): Promise<FileInfo> {
	const body: RenameRequest = { newPath };
	return api.put<FileInfo>(`/files/${encodeRoutePath(oldPath)}`, body);
}

/**
 * Save file content
 * PATCH /api/v1/files/*path
 */
export async function saveFileContent(path: string, content: string): Promise<FileInfo> {
	const body: SaveFileRequest = { content };
	return api.patch<FileInfo>(`/files/${encodeRoutePath(path)}`, body);
}

/**
 * Delete a file or directory
 * DELETE /api/v1/files/*path
 * @param confirm - Set to true to confirm directory deletion
 */
export async function deleteFile(path: string, confirm: boolean = false): Promise<MessageResponse> {
	const params = confirm ? { confirm: 'true' } : undefined;
	return api.delete<MessageResponse>(`/files/${encodeRoutePath(path)}`, params);
}

/**
 * Search for files by name
 * GET /api/v1/search?path=&q=
 */
export async function search(
	path: string,
	query: string,
	signal?: AbortSignal
): Promise<SearchResponse> {
	return api.get<SearchResponse>('/search', { path, q: query }, signal);
}

/**
 * Get a small cached image suitable for file-grid thumbnails.
 */
export function getThumbnailUrl(path: string, width = 224, height = 144, version?: string): string {
	const token = tokenStorage.getAccessToken();
	const encodedPath = encodeRoutePath(path);
	const params = new URLSearchParams({ width: String(width), height: String(height) });
	if (version !== undefined) params.set('v', version);
	if (token) params.set('token', token);
	return `/api/v1/stream/thumbnail/${encodedPath}?${params.toString()}`;
}

/**
 * Get the preview URL for a file (for streaming media, images, etc.)
 * This URL can be used directly in <video>, <audio>, <img>, <iframe> src
 */
export function getPreviewUrl(path: string): string {
	const token = tokenStorage.getAccessToken();
	const encodedPath = encodeRoutePath(path);
	const baseUrl = `/api/v1/stream/preview/${encodedPath}`;
	return token ? `${baseUrl}?token=${encodeURIComponent(token)}` : baseUrl;
}

/**
 * Get the download URL for a file
 */
export function getDownloadUrl(path: string): string {
	const token = tokenStorage.getAccessToken();
	const encodedPath = encodeRoutePath(path);
	const baseUrl = `/api/v1/stream/download/${encodedPath}`;
	return token ? `${baseUrl}?token=${encodeURIComponent(token)}` : baseUrl;
}

/** Get the download URL for a folder ZIP archive. */
export function getDirectoryArchiveUrl(path: string): string {
	const token = tokenStorage.getAccessToken();
	const encodedPath = encodeRoutePath(path);
	const baseUrl = `/api/v1/stream/archive/${encodedPath}`;
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
