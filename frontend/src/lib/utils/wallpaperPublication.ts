import { storage } from './storage.js';

export type WallpaperPublication =
	{ kind: 'absent' } | { kind: 'pending'; source: string } | { kind: 'completed' };

const publications = new Map<string, WallpaperPublication>();

function key(user: string): string {
	return `boxbox_wallpaper_pending:${encodeURIComponent(user)}`;
}

export function wallpaperPublication(user: string): WallpaperPublication {
	const current = publications.get(user);
	if (current) return current;
	try {
		const source = storage.getString(key(user));
		if (source === null) return { kind: 'absent' };
		return source === '' ? { kind: 'completed' } : { kind: 'pending', source };
	} catch {
		return { kind: 'absent' };
	}
}

export function recordWallpaperPublication(
	user: string,
	publication: Exclude<WallpaperPublication, { kind: 'absent' }>
): void {
	publications.set(user, publication);
	try {
		// Empty cannot be an image source. Retain completion instead of deleting
		// the key: failed cleanup must never reveal an older pending intent.
		storage.setString(key(user), publication.kind === 'pending' ? publication.source : '');
	} catch {
		// Keep acknowledged completion in memory when browser writes are unavailable.
	}
}
