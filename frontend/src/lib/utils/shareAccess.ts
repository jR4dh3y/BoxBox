import type { SharePermissions } from '#lib/api/shares.js';

export function getShareAccessLabel(permissions: SharePermissions): string {
	if (!permissions.upload) return 'View only';
	if (permissions.manage) return 'Full access';
	if (permissions.delete) return 'Upload + delete';
	return permissions.canReplace ? 'Upload + replace' : 'Upload only';
}

/**
 * Whether the share dialog offers to delete the file when its link expires.
 * The server enforces the same rules, including nested read-only mounts.
 */
export function offersDeleteOnExpiry(target: {
	isDir: boolean;
	readOnly: boolean;
	expiresInSeconds: number | undefined;
}): boolean {
	return !target.isDir && !target.readOnly && target.expiresInSeconds !== undefined;
}
