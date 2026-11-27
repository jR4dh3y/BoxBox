/**
 * Where the app opens without a folder in its address: the drives, or the folder the user was
 * last in, as the desktop app's Start location setting chooses.
 */
import { storage } from '#lib/utils/storage.js';
import { currentUsername } from '#lib/utils/sessionUser.js';

export type StartLocation = 'overview' | 'last';

export const START_LOCATION_OPTIONS: { value: StartLocation; label: string }[] = [
	{ value: 'overview', label: 'Drives' },
	{ value: 'last', label: 'Last location' }
];

function key(): string | null {
	const user = currentUsername();
	return user === null ? null : `boxbox_last_location:${user}`;
}

/** Kept apart from the settings, so moving between folders does not rewrite them. */
export function rememberLocation(path: string): void {
	const name = key();
	if (name === null) return;
	try {
		storage.setString(name, path);
	} catch {
		// The next start opens the drives instead.
	}
}

export function lastLocation(): string {
	const name = key();
	if (name === null) return '';
	try {
		return storage.getString(name) ?? '';
	} catch {
		return '';
	}
}
