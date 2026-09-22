/**
 * Share links API module
 * File and folder share links with token-authenticated recipient access
 */

import { api, apiRequest } from './client';

/**
 * Permissions granted to a share recipient
 */
export interface SharePermissions {
	view: boolean;
	download: boolean;
	write: boolean;
}

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
	/** Legacy file permission payload; folder shares may use explicit permissions. */
	permissions?: SharePermissions;
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
	expiresAt?: string;
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

interface RevokeShareResponse {
	success: boolean;
}

/**
 * Create a share link for a file or folder
 * POST /api/v1/shares
 */
export async function createShare(
	path: string,
	options: CreateShareOptions
): Promise<CreateShareResponse> {
	return api.post<CreateShareResponse>('/shares', {
		path,
		...(options.permissions ? { permissions: options.permissions } : {}),
		expiresInSeconds: options.expiresInSeconds ?? null
	});
}

/**
 * List active share links
 * GET /api/v1/shares
 */
export async function listShares(): Promise<ShareListResponse> {
	return api.get<ShareListResponse>('/shares');
}

/**
 * Revoke a share link
 * DELETE /api/v1/shares/{id}
 */
export async function revokeShare(id: string): Promise<RevokeShareResponse> {
	return api.delete<RevokeShareResponse>(`/shares/${encodeURIComponent(id)}`);
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

/**
 * Inline preview URL for a shared file (browser media streaming with ranges)
 */
export function sharePreviewUrl(token: string, path?: string): string {
	const query = path ? `?path=${encodeURIComponent(path)}` : '';
	return `/api/v1/share/${encodeURIComponent(token)}/preview${query}`;
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
