/**
 * Share links API module
 * Single-file share links with token-authenticated recipient access
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
	permissions: SharePermissions;
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
	permissions: SharePermissions;
	expiresAt?: string;
}

interface RevokeShareResponse {
	success: boolean;
}

/**
 * Create a share link for a single file
 * POST /api/v1/shares
 */
export async function createShare(
	path: string,
	options: CreateShareOptions
): Promise<CreateShareResponse> {
	return api.post<CreateShareResponse>('/shares', {
		path,
		permissions: options.permissions,
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
 * URL of the public recipient page for a share
 */
export function sharePageUrl(token: string): string {
	return `/s/${encodeURIComponent(token)}`;
}

/**
 * Download URL for a shared file (attachment disposition)
 */
export function shareDownloadUrl(token: string): string {
	return `/api/v1/share/${encodeURIComponent(token)}/download`;
}

/**
 * Inline preview URL for a shared file (browser media streaming with ranges)
 */
export function sharePreviewUrl(token: string): string {
	return `/api/v1/share/${encodeURIComponent(token)}/preview`;
}

/**
 * Overwrite URL for a shared file
 */
export function shareUploadUrl(token: string): string {
	return `/api/v1/share/${encodeURIComponent(token)}/upload`;
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
