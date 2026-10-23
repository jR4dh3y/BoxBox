/**
 * User preferences, kept in localStorage; drive names are also stored on the server
 */

import { settingsStorage, storage } from '#lib/utils/storage.js';
import { currentUsername } from '#lib/utils/sessionUser.js';
import {
	getDriveNames,
	setDriveName as apiSetDriveName,
	deleteDriveName as apiDeleteDriveName
} from '#lib/api/drive-names.js';
import {
	DEFAULT_BACKGROUND_IMAGE_MODE,
	normalizeBackgroundImageMode,
	type BackgroundImageMode
} from '#lib/utils/wallpaper.js';

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
	previewOnSingleClick: boolean;
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
	previewOnSingleClick: false,
	compactMode: false,
	driveNameOverrides: {},
	favoriteFolders: []
};

function loadSettings(username: string): UserSettings {
	// Only migrate legacy preferences when the previous sync recorded their owner.
	let stored = settingsStorage.get<Partial<UserSettings>>(username);
	if (!stored) {
		try {
			if (storage.getString('boxbox_wallpaper_owner') === username) {
				stored = settingsStorage.get<Partial<UserSettings>>();
			}
		} catch {
			// Browsers can disable localStorage even when the session is valid.
		}
	}
	const settings = { ...defaultSettings, ...stored };
	return {
		...settings,
		backgroundImageMode: normalizeBackgroundImageMode(settings.backgroundImageMode)
	};
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

class SettingsStore {
	current = $state.raw<UserSettings>({ ...defaultSettings });
	private username: string | null = null;
	private session = 0;

	/** Call after sign-in: the drive names come from an authenticated endpoint. */
	async initialize(): Promise<void> {
		const session = ++this.session;
		const username = currentUsername() ?? 'dev';
		this.username = username;
		this.current = loadSettings(username);

		const driveNameOverrides = await loadDriveNames();
		if (session !== this.session) return;
		this.current = { ...this.current, driveNameOverrides };
		try {
			settingsStorage.set(this.current, username);
		} catch {
			// Initialization should not fail auth because local preference persistence is full.
		}
	}

	set(settings: UserSettings): void {
		if (this.username !== null) settingsStorage.set(settings, this.username);
		this.current = settings;
	}

	clearSession(): void {
		this.session++;
		this.username = null;
		this.current = { ...defaultSettings };
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
