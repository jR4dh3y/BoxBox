/**
 * Share links API module
 * File and folder share links with token-authenticated recipient access
 */

import { api, apiRequest } from './client';
import { array, boolean, optionalString, record, string } from './parse';
import {
	collectRemote,
	listRemoteServers,
	number,
	parseRemotePath,
	pathEndpoint,
	qualifyPath,
	remoteSource,
	sourceEndpoint,
	type PathReference,
	type RemoteServer
} from './remotePaths';
import type { BackgroundImageMode } from '#lib/utils/wallpaper.js';

/**
 * Permissions granted to a share recipient
 */
export interface SharePermissions {
	view: boolean;
	download: boolean;
	upload: boolean;
	delete: boolean;
	/** Full access: also create folders and edit files. Requires upload and delete. */
	manage: boolean;
	canReplace: boolean;
}

export type SharePermissionInput = Pick<
	SharePermissions,
	'view' | 'download' | 'upload' | 'delete' | 'manage'
>;

/**
 * Active share in the owner's share list
 */
export interface ShareRecord {
	id: string;
	token: string;
	url: string;
	fileName: string;
	path: string;
	isFolder: boolean;
	permissions: SharePermissions;
	maxUploadBytes: number;
	createdAt: string;
	expiresAt?: string;
}

/**
 * Share list response
 */
export interface ShareListResponse {
	shares: ShareRecord[];
}

/**
 * Options when creating a share link
 */
export interface CreateShareOptions {
	/** File shares always use view/download; folder shares can also upload or delete. */
	permissions?: SharePermissionInput;
	/** Optional per-file upload cap; omitted means the server's configured maximum. */
	maxUploadBytes?: number;
	/** Seconds until the share expires; omitted means it never expires */
	expiresInSeconds?: number;
}

/**
 * Response when a share link is created
 */
export interface CreateShareResponse {
	id: string;
	token: string;
	url: string;
	fileName: string;
	isFolder: boolean;
	permissions: SharePermissions;
	maxUploadBytes: number;
	createdAt: string;
	expiresAt?: string;
}

/**
 * Recipient-facing share metadata (no internal paths)
 */
export interface ShareInfoResponse {
	fileName: string;
	size: number;
	mimeType: string;
	isFolder: boolean;
	permissions: SharePermissions;
	maxUploadBytes: number;
	expiresAt?: string;
	/** The share owner's wallpaper; absent when they have none. */
	wallpaper?: {
		mode: BackgroundImageMode;
		frostedGlass: boolean;
		version: number;
	};
}

/**
 * One entry in a shared folder. Paths are relative to the shared folder.
 */
export interface ShareItem {
	name: string;
	path: string;
	size: number;
	isDir: boolean;
	modTime: string;
	mimeType?: string;
}

export interface ShareDirectoryResponse {
	path: string;
	items: ShareItem[];
}

interface ShareActionResponse {
	success: boolean;
}

function parsePermissions(value: unknown): SharePermissions {
	const p = record(value, 'share permissions');
	return {
		view: boolean(p.view, 'view'),
		download: boolean(p.download, 'download'),
		upload: boolean(p.upload, 'upload'),
		delete: boolean(p.delete, 'delete'),
		manage: boolean(p.manage, 'manage'),
		canReplace: boolean(p.canReplace, 'replace')
	};
}

function parseCreatedShare(
	value: unknown,
	source: PathReference,
	server?: RemoteServer
): CreateShareResponse {
	const share = record(value, 'share');
	const url = string(share.url, 'share URL');
	const token = string(share.token, 'share token');
	return {
		id: qualifyPath(source, string(share.id, 'share ID')),
		token,
		url: server
			? new URL(
					`s/${encodeURIComponent(token)}`,
					server.url.endsWith('/') ? server.url : `${server.url}/`
				).href
			: url,
		fileName: string(share.fileName, 'file name'),
		isFolder: boolean(share.isFolder, 'folder flag'),
		permissions: parsePermissions(share.permissions),
		maxUploadBytes: number(share.maxUploadBytes, 'upload limit'),
		createdAt: string(share.createdAt, 'created time'),
		expiresAt: optionalString(share.expiresAt, 'expiry')
	};
}

function parseShares(value: unknown, source: PathReference, server?: RemoteServer): ShareRecord[] {
	return array(record(value, 'shares response').shares, 'shares').map((value) => ({
		...parseCreatedShare(value, source, server),
		path: qualifyPath(source, string(record(value, 'share').path, 'share path'))
	}));
}

/**
 * Create a share link for a file or folder
 * POST /api/v1/shares
 */
export async function createShare(
	path: string,
	options: CreateShareOptions
): Promise<CreateShareResponse> {
	const source = parseRemotePath(path);
	const serverPromise =
		source.kind === 'remote'
			? listRemoteServers().then((servers) => {
					const server = servers.find((server) => server.id === source.serverId);
					if (!server) throw new Error('Remote server is unavailable');
					return server;
				})
			: Promise.resolve(undefined);
	const [server, value] = await Promise.all([
		serverPromise,
		api.post<unknown>(sourceEndpoint(source, '/shares'), {
			path: source.path,
			...(options.permissions ? { permissions: options.permissions } : {}),
			...(options.maxUploadBytes !== undefined ? { maxUploadBytes: options.maxUploadBytes } : {}),
			expiresInSeconds: options.expiresInSeconds ?? null
		})
	]);
	return parseCreatedShare(value, source, server);
}

/**
 * List active share links
 * GET /api/v1/shares
 */
export async function listShares(): Promise<ShareListResponse> {
	const [local, remote] = await Promise.all([
		api.get<unknown>('/shares').then((value) => parseShares(value, { kind: 'local', path: '' })),
		collectRemote(async (server) =>
			parseShares(
				await api.get<unknown>(sourceEndpoint(remoteSource(server), '/shares')),
				remoteSource(server),
				server
			)
		)
	]);
	return { shares: [...local, ...remote] };
}

/**
 * Revoke a share link
 * DELETE /api/v1/shares/{id}
 */
export async function revokeShare(id: string): Promise<ShareActionResponse> {
	const value = await apiRequest<unknown>(pathEndpoint(id, '/shares'), { method: 'DELETE' });
	return value === undefined
		? { success: true }
		: { success: boolean(record(value, 'revoke response').success, 'success') };
}

/** Remove an item below a shared folder. The token is the only credential. */
export async function deleteShareItem(token: string, path: string): Promise<ShareActionResponse> {
	return apiRequest<ShareActionResponse>(`/share/${encodeURIComponent(token)}/items`, {
		method: 'DELETE',
		skipAuth: true,
		params: { path }
	});
}

/** Create a folder (and missing parents) below a full-access shared folder. */
export async function createShareFolder(
	token: string,
	path: string,
	signal?: AbortSignal
): Promise<ShareActionResponse> {
	return apiRequest<ShareActionResponse>(`/share/${encodeURIComponent(token)}/folders`, {
		method: 'POST',
		skipAuth: true,
		signal,
		params: { path }
	});
}

/**
 * Fetch recipient-facing share metadata
 * GET /api/v1/share/{token}
 * No authentication required; the token in the URL is the only credential.
 */
export async function getShareInfo(token: string): Promise<ShareInfoResponse> {
	return apiRequest<ShareInfoResponse>(`/share/${encodeURIComponent(token)}`, { skipAuth: true });
}

/**
 * List a shared folder path. The path is relative to the shared folder root.
 */
export async function listShareItems(token: string, path = ''): Promise<ShareDirectoryResponse> {
	const query = path ? `?path=${encodeURIComponent(path)}` : '';
	return apiRequest<ShareDirectoryResponse>(`/share/${encodeURIComponent(token)}/items${query}`, {
		skipAuth: true
	});
}

/**
 * URL of the public recipient page for a share
 */
export function sharePageUrl(token: string): string {
	return `/s/${encodeURIComponent(token)}`;
}

/**
 * Download URL for a shared file (attachment disposition)
 */
export function shareDownloadUrl(token: string, path?: string): string {
	const query = path ? `?path=${encodeURIComponent(path)}` : '';
	return `/api/v1/share/${encodeURIComponent(token)}/download${query}`;
}

/** Download a shared folder or subfolder as a ZIP archive. */
export function shareArchiveUrl(token: string, path?: string): string {
	const query = path ? `?path=${encodeURIComponent(path)}` : '';
	return `/api/v1/share/${encodeURIComponent(token)}/archive${query}`;
}

/**
 * Inline preview URL for a shared file (browser media streaming with ranges)
 */
export function sharePreviewUrl(token: string, path?: string): string {
	const query = path ? `?path=${encodeURIComponent(path)}` : '';
	return `/api/v1/share/${encodeURIComponent(token)}/preview${query}`;
}

/** The share owner's wallpaper image. Version busts the cache when it changes. */
export function shareWallpaperUrl(token: string, version: number): string {
	return `/api/v1/share/${encodeURIComponent(token)}/wallpaper?v=${version}`;
}

/**
 * Upload URL for a file below a shared folder
 */
export function shareUploadUrl(token: string, path?: string): string {
	const query = path ? `?path=${encodeURIComponent(path)}` : '';
	return `/api/v1/share/${encodeURIComponent(token)}/upload${query}`;
}

/**
 * Whether an expiry timestamp is a real deadline. The backend serializes
 * never-expiring shares with the zero time, which is not a valid deadline.
 */
export function hasShareExpiry(expiresAt?: string): expiresAt is string {
	if (!expiresAt) return false;
	const date = new Date(expiresAt);
	return !isNaN(date.getTime()) && date.getTime() > 0;
}
