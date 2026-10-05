/**
 * The signed-in username, read from the access token's `username` claim.
 */
import { getAccessToken } from '$lib/api/client';

export function currentUsername(): string | null {
	const token = getAccessToken();
	const payload = token?.split('.')[1];
	if (!payload) return null;
	try {
		const json = atob(payload.replace(/-/g, '+').replace(/_/g, '/'));
		const claims: unknown = JSON.parse(json);
		if (typeof claims === 'object' && claims !== null && 'username' in claims) {
			const { username } = claims;
			return typeof username === 'string' && username !== '' ? username : null;
		}
		return null;
	} catch {
		return null;
	}
}
