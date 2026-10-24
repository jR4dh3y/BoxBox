import { api, apiRequest } from './client';
import { array, boolean, isRecord, record, string } from './parse';
import type { UserSettings } from '#lib/stores/settings.svelte.js';
import { normalizeFileOpenBehavior } from '#lib/utils/fileOpenBehavior.js';
import { normalizeBackgroundImageMode } from '#lib/utils/wallpaper.js';

function choice<T extends string>(value: unknown, choices: T[], fallback: T): T {
	if (value === undefined) return fallback;
	const match = choices.find((choice) => choice === value);
	if (match === undefined) throw new Error('Invalid desktop preference');
	return match;
}

/** A saved desktop file is external JSON, even though only this renderer writes it. */
export function parseDesktopPreferences(value: unknown, defaults: UserSettings): UserSettings {
	const saved = record(value, 'desktop preferences');
	const flag = (key: keyof UserSettings) =>
		saved[key] === undefined ? boolean(defaults[key], key) : boolean(saved[key], key);
	const nullable = (key: 'accentColor' | 'backgroundImage') =>
		saved[key] === undefined ? defaults[key] : saved[key] === null ? null : string(saved[key], key);
	return {
		showHiddenFiles: flag('showHiddenFiles'),
		showFileExtensions: flag('showFileExtensions'),
		confirmDelete: flag('confirmDelete'),
		defaultSortBy: choice(
			saved.defaultSortBy,
			['name', 'size', 'modTime', 'type'],
			defaults.defaultSortBy
		),
		defaultSortDir: choice(saved.defaultSortDir, ['asc', 'desc'], defaults.defaultSortDir),
		defaultViewMode: choice(saved.defaultViewMode, ['list', 'grid'], defaults.defaultViewMode),
		accentColor: nullable('accentColor'),
		backgroundImage: nullable('backgroundImage'),
		backgroundImageMode: normalizeBackgroundImageMode(saved.backgroundImageMode),
		frostedGlass: flag('frostedGlass'),
		fileOpenBehavior: normalizeFileOpenBehavior(saved.fileOpenBehavior, saved.previewOnSingleClick),
		compactMode: flag('compactMode'),
		driveNameOverrides: isRecord(saved.driveNameOverrides)
			? Object.fromEntries(
					Object.entries(saved.driveNameOverrides).map(([key, value]) => [
						key,
						string(value, 'drive name')
					])
				)
			: {},
		favoriteFolders:
			saved.favoriteFolders === undefined
				? []
				: array(saved.favoriteFolders, 'favorites').map((value) => {
						const folder = record(value, 'favorite');
						return {
							name: string(folder.name, 'favorite name'),
							path: string(folder.path, 'favorite path')
						};
					})
	};
}

export async function loadDesktopPreferences(defaults: UserSettings): Promise<UserSettings | null> {
	const value = await api.get<unknown>('/desktop/preferences');
	return value === null ? null : parseDesktopPreferences(value, defaults);
}

export async function saveDesktopPreferences(settings: UserSettings): Promise<void> {
	await api.put('/desktop/preferences', settings);
}

export async function saveDesktopWallpaper(image: Blob): Promise<string> {
	await apiRequest('/desktop/wallpaper', { method: 'PUT', body: image });
	return `/api/v1/desktop/wallpaper?v=${Date.now()}`;
}
