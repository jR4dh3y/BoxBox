import type { DeploymentIcon } from '#lib/desktop/deploymentIcon.js';
/**
 * File API module for file operations
 */

import { api, buildUrl } from './client';
import { array, boolean, record, string, optionalString } from './parse';
import {
	number,
	parseRemotePath,
	pathEndpoint,
	qualifyPath,
	remoteSource,
	sameSource,
	sourceEndpoint,
	type RemoteServer,
	type PathReference
} from './remotePaths';
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
	label?: string;
	serverName?: string;
	serverIcon?: DeploymentIcon;
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
	label?: string;
	serverName?: string;
	serverIcon?: DeploymentIcon;
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

function parseFile(value: unknown, source: PathReference): FileInfo {
	const file = record(value, 'file');
	return {
		name: string(file.name, 'file name'),
		path: qualifyPath(source, string(file.path, 'file path')),
		size: number(file.size, 'file size'),
		isDir: boolean(file.isDir, 'directory flag'),
		modTime: string(file.modTime, 'modified time'),
		permissions: string(file.permissions, 'permissions'),
		mimeType: optionalString(file.mimeType, 'MIME type')
	};
}

function parseRoots(
	value: unknown,
	source: PathReference,
	serverName?: string,
	serverIcon?: DeploymentIcon
): MountPoint[] {
	return array(record(value, 'roots response').roots, 'roots').map((value) => {
		const root = record(value, 'root');
		const name = string(root.name, 'root name');
		const kind = root.kind === '' ? undefined : root.kind;
		if (kind !== undefined && kind !== 'drive' && kind !== 'place')
			throw new Error('Invalid root kind');
		return {
			name: qualifyPath(source, name),
			path: source.kind === 'remote' ? qualifyPath(source, name) : string(root.path, 'root path'),
			readOnly: boolean(root.readOnly, 'read-only flag'),
			...(root.autoDiscover === undefined
				? {}
				: { autoDiscover: boolean(root.autoDiscover, 'auto-discover flag') }),
			...(kind === undefined ? {} : { kind }),
			...(serverName === undefined ? {} : { label: name, serverName, serverIcon })
		};
	});
}

function parseStats(
	value: unknown,
	source: PathReference,
	serverName?: string,
	serverIcon?: DeploymentIcon
): DriveStats[] {
	return array(record(value, 'stats response').drives, 'drives').map((value) => {
		const drive = record(value, 'drive');
		const name = string(drive.name, 'drive name');
		return {
			name: qualifyPath(source, name),
			path: source.kind === 'remote' ? qualifyPath(source, name) : string(drive.path, 'drive path'),
			totalBytes: number(drive.totalBytes, 'total bytes'),
			freeBytes: number(drive.freeBytes, 'free bytes'),
			usedBytes: number(drive.usedBytes, 'used bytes'),
			usedPct: number(drive.usedPct, 'used percentage'),
			readOnly: boolean(drive.readOnly, 'read-only flag'),
			...(source.kind === 'remote'
				? { label: name, serverName, serverIcon }
				: {
						device: optionalString(drive.device, 'device'),
						fsType: optionalString(drive.fsType, 'filesystem'),
						mountPoint: optionalString(drive.mountPoint, 'mount point')
					})
		};
	});
}

/**
 * List all configured mount points (roots)
 * GET /api/v1/files
 */
export async function listRoots(
	server?: RemoteServer,
	signal?: AbortSignal
): Promise<RootsResponse> {
	const source: PathReference = server ? remoteSource(server) : { kind: 'local', path: '' };
	const value = await api.get<unknown>(sourceEndpoint(source, '/files'), undefined, signal);
	return { roots: parseRoots(value, source, server?.name, server?.icon) };
}

/** Statistics for one host. Remote hosts never hold up the local catalog. */
export async function getDriveStats(
	server?: RemoteServer,
	signal?: AbortSignal
): Promise<DriveStatsResponse> {
	const source: PathReference = server ? remoteSource(server) : { kind: 'local', path: '' };
	const value = await api.get<unknown>(sourceEndpoint(source, '/files/stats'), undefined, signal);
	return { drives: parseStats(value, source, server?.name, server?.icon) };
}

function listEndpoint(path: string): string {
	return pathEndpoint(path, '/files/list');
}

/** Every entry of a directory in one response. The client sorts and filters. */
export async function listDirectory(path: string, signal?: AbortSignal): Promise<FileList> {
	const listing = parseDirectory(await api.get<unknown>(listEndpoint(path), undefined, signal));
	const source = parseRemotePath(path);
	if (source.kind === 'local') return listing;
	return {
		path: qualifyPath(source, listing.path),
		items: listing.items.map((item) => ({ ...item, path: qualifyPath(source, item.path) }))
	};
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
	return parseFile(await api.get<unknown>(pathEndpoint(path, '/files')), parseRemotePath(path));
}

/**
 * Create a new directory
 * POST /api/v1/files/*path
 */
export async function createDirectory(basePath: string, name: string): Promise<FileInfo> {
	const body: CreateItemRequest = { name, type: 'directory' };
	return parseFile(
		await api.post<unknown>(pathEndpoint(basePath, '/files'), body),
		parseRemotePath(basePath)
	);
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
	return parseFile(
		await api.post<unknown>(pathEndpoint(basePath, '/files'), body),
		parseRemotePath(basePath)
	);
}

/**
 * Rename or move a file/directory
 * PUT /api/v1/files/*path
 */
export async function rename(oldPath: string, newPath: string): Promise<FileInfo> {
	const source = parseRemotePath(oldPath);
	const destination = parseRemotePath(newPath);
	if (!sameSource(source, destination)) throw new Error('Rename requires the same source');
	const body: RenameRequest = { newPath: destination.path };
	return parseFile(await api.put<unknown>(pathEndpoint(oldPath, '/files'), body), source);
}

/**
 * Save file content
 * PATCH /api/v1/files/*path
 */
export async function saveFileContent(path: string, content: string): Promise<FileInfo> {
	const body: SaveFileRequest = { content };
	return parseFile(
		await api.patch<unknown>(pathEndpoint(path, '/files'), body),
		parseRemotePath(path)
	);
}

/**
 * Delete a file or directory
 * DELETE /api/v1/files/*path
 * @param confirm - Set to true to confirm directory deletion
 */
export async function deleteFile(path: string, confirm: boolean = false): Promise<MessageResponse> {
	const params = confirm ? { confirm: 'true' } : undefined;
	const response = record(
		await api.delete<unknown>(pathEndpoint(path, '/files'), params),
		'delete response'
	);
	return { message: string(response.message, 'message') };
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
	const source = parseRemotePath(path);
	const response = record(
		await api.get<unknown>(
			sourceEndpoint(source, '/search'),
			{ path: source.path, q: query },
			signal
		),
		'search response'
	);
	return {
		path: qualifyPath(source, string(response.path, 'search path')),
		query: string(response.query, 'query'),
		results: array(response.results, 'search results').map((file) => parseFile(file, source)),
		count: number(response.count, 'count')
	};
}

/**
 * Get a small cached image suitable for file-grid thumbnails.
 */
export function getThumbnailUrl(path: string, width = 224, height = 144, version?: string): string {
	const token = tokenStorage.getAccessToken();
	const source = parseRemotePath(path);
	const params = new URLSearchParams({ width: String(width), height: String(height) });
	if (version !== undefined) params.set('v', version);
	if (token && source.kind === 'local') params.set('token', token);
	return `/api/v1${pathEndpoint(path, '/stream/thumbnail')}?${params.toString()}`;
}

/**
 * Get the preview URL for a file (for streaming media, images, etc.)
 * This URL can be used directly in <video>, <audio>, <img>, <iframe> src
 */
export function getPreviewUrl(path: string): string {
	const token = tokenStorage.getAccessToken();
	const source = parseRemotePath(path);
	const baseUrl = `/api/v1${pathEndpoint(path, '/stream/preview')}`;
	return token && source.kind === 'local'
		? `${baseUrl}?token=${encodeURIComponent(token)}`
		: baseUrl;
}

/**
 * Get the download URL for a file
 */
export function getDownloadUrl(path: string): string {
	const token = tokenStorage.getAccessToken();
	const source = parseRemotePath(path);
	const baseUrl = `/api/v1${pathEndpoint(path, '/stream/download')}`;
	return token && source.kind === 'local'
		? `${baseUrl}?token=${encodeURIComponent(token)}`
		: baseUrl;
}

/** Get the download URL for a folder ZIP archive. */
export function getDirectoryArchiveUrl(path: string): string {
	const token = tokenStorage.getAccessToken();
	const source = parseRemotePath(path);
	const baseUrl = `/api/v1${pathEndpoint(path, '/stream/archive')}`;
	return token && source.kind === 'local'
		? `${baseUrl}?token=${encodeURIComponent(token)}`
		: baseUrl;
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
