/**
 * User preferences, kept in localStorage; drive names are also stored on the server
 */

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
	saveLocalWallpaperDataUrl
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
	settingsStorage.set(settings);
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
	current = $state.raw<UserSettings>(loadSettings());

	/** Call after sign-in: the drive names come from an authenticated endpoint. */
	async initialize(): Promise<void> {
		const migrated = await migrateInlineBackgroundImage(this.current);
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
	}

	set(settings: UserSettings): void {
		saveSettings(settings);
		this.current = settings;
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
