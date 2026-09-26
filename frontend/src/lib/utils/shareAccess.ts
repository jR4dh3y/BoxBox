import type { SharePermissions } from '$lib/api/shares';

export function getShareAccessLabel(permissions: SharePermissions): string {
	if (!permissions.upload) return 'View only';
	if (permissions.delete) return 'Upload + delete';
	return permissions.canReplace ? 'Upload + replace' : 'Upload only';
}
