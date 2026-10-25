import { storage } from './storage.js';

export type WallpaperPublication =
	| { kind: 'absent' }
	| { kind: 'pending'; id: string; source: string | null }
	| { kind: 'completed'; id: string };

// A fallback is valid only while the durable intent has not changed underneath it.
const publications = new Map<string, { raw: string | null; state: WallpaperPublication }>();

function key(user: string): string {
	return `boxbox_wallpaper_pending:${encodeURIComponent(user)}`;
}

function parse(raw: string | null): WallpaperPublication {
	if (raw === null) return { kind: 'absent' };
	if (raw === '') return { kind: 'completed', id: '' };
	if (!raw.startsWith('{')) return { kind: 'pending', id: raw, source: raw };
	try {
		const value: unknown = JSON.parse(raw);
		if (
			typeof value === 'object' &&
			value !== null &&
			'id' in value &&
			typeof value.id === 'string' &&
			'source' in value &&
			(typeof value.source === 'string' || value.source === null)
		)
			return { kind: 'pending', id: value.id, source: value.source };
	} catch {
		/* Invalid browser data is not publication intent. */
	}
	return { kind: 'absent' };
}

export function wallpaperPublication(user: string): WallpaperPublication {
	const cached = publications.get(user);
	try {
		const raw = storage.getString(key(user));
		let state = cached && cached.raw === raw ? cached.state : parse(raw);
		if (state.kind === 'pending') {
			try {
				if (storage.getString(`${key(user)}:${state.id}`) !== null)
					state = { kind: 'completed', id: state.id };
			} catch {
				/* Unreadable acknowledgement cannot discard a new intent. */
			}
		}
		publications.set(user, { raw, state });
		return state;
	} catch {
		return cached?.state ?? { kind: 'absent' };
	}
}

/** Record synchronously with local apply, before any paint, queue or transport. */
export function beginWallpaperPublication(user: string, source: string | null): string {
	const previous = wallpaperPublication(user);
	let raw = publications.get(user)?.raw ?? null;
	const id = crypto.getRandomValues(new Uint32Array(4)).join('-');
	try {
		const next = JSON.stringify({ id, source });
		storage.setString(key(user), next);
		raw = next;
		if (previous.kind === 'completed') storage.remove(`${key(user)}:${previous.id}`);
	} catch {
		/* Keep the applied intent usable when browser writes fail. */
	}
	publications.set(user, { raw, state: { kind: 'pending', id, source } });
	return id;
}

export function completeWallpaperPublication(user: string, id: string): void {
	// An acknowledgement owns its receipt, never another tab's current intent.
	try {
		storage.setString(`${key(user)}:${id}`, '');
	} catch {
		/* Retain acknowledgement in memory if its durable write fails. */
	}
	const current = wallpaperPublication(user);
	if (current.kind === 'pending' && current.id === id) {
		const cached = publications.get(user);
		if (cached) cached.state = { kind: 'completed', id };
	}
}
