import { toastStore } from '#lib/stores/toast.svelte.js';
import { api, apiRequest } from './client';
import { array, boolean, isRecord, record, string } from './parse';
import type { UserSettings } from '#lib/stores/settings.svelte.js';
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
	const nullable = (key: 'accentColor' | 'backgroundImage' | 'backgroundImageName') =>
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
		accentColorSource: choice(
			saved.accentColorSource,
			['manual', 'wallpaper'],
			defaults.accentColorSource
		),
		backgroundImage: nullable('backgroundImage'),
		backgroundImageName: nullable('backgroundImageName'),
		backgroundImageMode: normalizeBackgroundImageMode(saved.backgroundImageMode),
		frostedGlass: flag('frostedGlass'),
		wallpaperTint: flag('wallpaperTint'),
		hardwareAcceleration: flag('hardwareAcceleration'),
		previewOnSingleClick: flag('previewOnSingleClick'),
		selectionCheckboxes: flag('selectionCheckboxes'),
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

export async function saveDesktopWallpaper(image: Blob, signal: AbortSignal): Promise<string> {
	const value = await apiRequest<unknown>('/desktop/wallpaper', {
		method: 'PUT',
		body: image,
		signal
	});
	const id = string(record(value, 'desktop wallpaper').sha256, 'wallpaper hash');
	if (!/^[a-f0-9]{64}$/.test(id)) throw new Error('Invalid desktop wallpaper reference');
	return `/api/v1/desktop/wallpaper/${id}`;
}

/** Move legacy renderer images into desktop storage before preferences are persisted. */
export async function migrateDesktopWallpaper(
	settings: UserSettings,
	signal: AbortSignal
): Promise<UserSettings> {
	const { normalizeBackgroundImage } = await import('#lib/utils/appearance.js');
	const {
		isInlineWallpaperDataUrl,
		isLocalWallpaperReference,
		dataUrlToBlob,
		getLocalWallpaperBlob
	} = await import('#lib/utils/wallpaperStorage.js');
	const backgroundImage = normalizeBackgroundImage(settings.backgroundImage);
	if (
		!backgroundImage ||
		(!isInlineWallpaperDataUrl(backgroundImage) && !isLocalWallpaperReference(backgroundImage))
	)
		return settings;
	const image = isInlineWallpaperDataUrl(backgroundImage)
		? dataUrlToBlob(backgroundImage)
		: await getLocalWallpaperBlob(backgroundImage);
	if (!image) throw new Error('The saved desktop wallpaper could not be read.');
	return { ...settings, backgroundImage: await saveDesktopWallpaper(image, signal) };
}

/** Desktop persistence shares the settings session's cancellation lifetime. */
export class DesktopPreferences {
	private loaded = false;
	private saving: Promise<void> = Promise.resolve();

	constructor(private readonly signal: AbortSignal) {}

	async load(defaults: UserSettings, fallback: UserSettings): Promise<UserSettings> {
		try {
			const saved = await loadDesktopPreferences(defaults);
			this.signal.throwIfAborted();
			const migrated = await migrateDesktopWallpaper(saved ?? fallback, this.signal);
			this.signal.throwIfAborted();
			this.loaded = true;
			// The server keeps only the staged wallpaper the file names, so adopt a fallback or migration now.
			if (saved === null || migrated !== saved) this.save(migrated);
			return migrated;
		} catch {
			if (!this.signal.aborted)
				toastStore.error(
					'Desktop preferences could not load. Reload to retry. Changes will not survive restart.'
				);
			return fallback;
		}
	}

	save(settings: UserSettings): void {
		if (!this.loaded) return;
		this.saving = this.saving
			.catch(() => {})
			.then(async () => {
				this.signal.throwIfAborted();
				await saveDesktopPreferences(settings);
			});
		void this.saving.catch(() => {
			if (!this.signal.aborted) toastStore.error('Desktop preferences could not be saved.');
		});
	}

	flush(): Promise<void> {
		return this.saving;
	}
}
