/**
 * User preferences use browser storage on the web and private backend storage on desktop.
 */

import { isLocalDesktop } from '#lib/api/desktop.js';
import { toastStore } from './toast.svelte';
import { settingsStorage } from '#lib/utils/storage.js';
import {
	getDriveNames,
	setDriveName as apiSetDriveName,
	deleteDriveName as apiDeleteDriveName
} from '#lib/api/drive-names.js';
import { normalizeBackgroundImage } from '#lib/utils/appearance.js';
import {
	DEFAULT_BACKGROUND_IMAGE_MODE,
	normalizeBackgroundImageMode,
	type BackgroundImageMode
} from '#lib/utils/wallpaper.js';
import { normalizeFileOpenBehavior, type FileOpenBehavior } from '#lib/utils/fileOpenBehavior.js';
import {
	isInlineWallpaperDataUrl,
	saveLocalWallpaperDataUrl,
	isLocalWallpaperReference,
	getLocalWallpaperBlob,
	dataUrlToBlob
} from '#lib/utils/wallpaperStorage.js';

export interface UserSettings {
	showHiddenFiles: boolean;
	showFileExtensions: boolean;
	confirmDelete: boolean;
	defaultSortBy: 'name' | 'size' | 'modTime' | 'type';
	defaultSortDir: 'asc' | 'desc';
	defaultViewMode: 'list' | 'grid';
	accentColor: string | null;
	backgroundImage: string | null;
	backgroundImageMode: BackgroundImageMode;
	frostedGlass: boolean;
	fileOpenBehavior: FileOpenBehavior;
	compactMode: boolean;
	driveNameOverrides: Record<string, string>;
	favoriteFolders: FavoriteFolder[];
}

export interface FavoriteFolder {
	name: string;
	path: string;
}

const defaultSettings: UserSettings = {
	showHiddenFiles: false,
	showFileExtensions: true,
	confirmDelete: true,
	defaultSortBy: 'name',
	defaultSortDir: 'asc',
	defaultViewMode: 'list',
	accentColor: null,
	backgroundImage: null,
	backgroundImageMode: DEFAULT_BACKGROUND_IMAGE_MODE,
	frostedGlass: false,
	fileOpenBehavior: 'double-click',
	compactMode: false,
	driveNameOverrides: {},
	favoriteFolders: []
};

interface StoredUserSettings extends Partial<UserSettings> {
	previewOnSingleClick?: unknown;
}

function loadSettings(): UserSettings {
	const stored = settingsStorage.get<StoredUserSettings>();
	const { previewOnSingleClick, ...storedSettings } = stored ?? {};
	const settings = {
		...defaultSettings,
		...storedSettings,
		fileOpenBehavior: normalizeFileOpenBehavior(
			storedSettings.fileOpenBehavior,
			previewOnSingleClick
		)
	};
	return {
		...settings,
		backgroundImageMode: normalizeBackgroundImageMode(settings.backgroundImageMode)
	};
}

function saveSettings(settings: UserSettings): void {
	try {
		settingsStorage.set(settings);
	} catch (error) {
		if (!isLocalDesktop()) throw error;
	}
}

async function loadDriveNames(): Promise<Record<string, string>> {
	try {
		const response = await getDriveNames();
		const names: Record<string, string> = {};
		for (const mapping of response.mappings) {
			names[mapping.mountPoint] = mapping.customName;
		}
		return names;
	} catch {
		return {};
	}
}

async function migrateInlineBackgroundImage(settings: UserSettings): Promise<UserSettings> {
	const backgroundImage = normalizeBackgroundImage(settings.backgroundImage);
	if (
		backgroundImage &&
		isLocalDesktop() &&
		(isInlineWallpaperDataUrl(backgroundImage) || isLocalWallpaperReference(backgroundImage))
	) {
		const image = isInlineWallpaperDataUrl(backgroundImage)
			? dataUrlToBlob(backgroundImage)
			: await getLocalWallpaperBlob(backgroundImage);
		if (!image) throw new Error('The saved desktop wallpaper could not be read.');
		const { saveDesktopWallpaper } = await import('#lib/api/desktopPreferences.js');
		return { ...settings, backgroundImage: await saveDesktopWallpaper(image) };
	}
	if (!backgroundImage || !isInlineWallpaperDataUrl(backgroundImage)) {
		return settings;
	}

	try {
		const migratedBackgroundImage = await saveLocalWallpaperDataUrl(backgroundImage);
		return {
			...settings,
			backgroundImage: migratedBackgroundImage
		};
	} catch {
		return settings;
	}
}

class SettingsStore {
	private desktopLoaded = false;
	private saving: Promise<void> = Promise.resolve();
	current = $state.raw<UserSettings>(loadSettings());

	/** Call after sign-in: the drive names come from an authenticated endpoint. */
	async initialize(): Promise<void> {
		if (isLocalDesktop() && !this.desktopLoaded) {
			try {
				const { loadDesktopPreferences } = await import('#lib/api/desktopPreferences.js');
				this.current = (await loadDesktopPreferences(defaultSettings)) ?? this.current;
				this.desktopLoaded = true;
			} catch {
				toastStore.error(
					'Desktop preferences could not load. Reload to retry. Changes will not survive restart.'
				);
			}
		}
		const migrated = await migrateInlineBackgroundImage(this.current).catch(() => {
			toastStore.error('The saved wallpaper could not be restored. Choose it again in Settings.');
			return this.current;
		});
		if (migrated !== this.current) {
			try {
				saveSettings(migrated);
			} catch {
				// Keep the in-memory migrated reference even if the browser refuses persistence.
			}
			this.current = migrated;
		}

		const driveNameOverrides = await loadDriveNames();
		this.current = { ...this.current, driveNameOverrides };
		try {
			saveSettings(this.current);
		} catch {
			// Initialization should not fail auth because local preference persistence is full.
		}
		if (this.desktopLoaded) {
			this.set(this.current);
			await this.flush().catch(() => {});
		}
	}

	flush(): Promise<void> {
		return this.saving;
	}

	set(settings: UserSettings): void {
		saveSettings(settings);
		this.current = settings;
		if (isLocalDesktop() && this.desktopLoaded) {
			this.saving = this.saving
				.catch(() => {})
				.then(async () => {
					const { saveDesktopPreferences } = await import('#lib/api/desktopPreferences.js');
					await saveDesktopPreferences(settings);
				});
			void this.saving.catch(() => {
				toastStore.error('Desktop preferences could not be saved.');
			});
		}
	}

	reset(): void {
		this.set(defaultSettings);
	}

	async setDriveName(originalName: string, customName: string): Promise<void> {
		await apiSetDriveName({ mountPoint: originalName, customName });
		this.set({
			...this.current,
			driveNameOverrides: { ...this.current.driveNameOverrides, [originalName]: customName }
		});
	}

	async removeDriveName(originalName: string): Promise<void> {
		await apiDeleteDriveName(originalName);
		const rest = { ...this.current.driveNameOverrides };
		delete rest[originalName];
		this.set({ ...this.current, driveNameOverrides: rest });
	}

	toggleHiddenFiles(): void {
		this.set({ ...this.current, showHiddenFiles: !this.current.showHiddenFiles });
	}

	pinFavoriteFolder(folder: FavoriteFolder): void {
		if (this.current.favoriteFolders.some((favorite) => favorite.path === folder.path)) return;
		this.set({ ...this.current, favoriteFolders: [...this.current.favoriteFolders, folder] });
	}

	unpinFavoriteFolder(path: string): void {
		this.set({
			...this.current,
			favoriteFolders: this.current.favoriteFolders.filter((folder) => folder.path !== path)
		});
	}
}

export const settingsStore = new SettingsStore();
