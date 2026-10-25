/**
 * User preferences use account-scoped browser storage on the web and private storage on desktop.
 */

import { isLocalDesktop } from '#lib/api/desktop.js';
import type { DesktopPreferences } from '#lib/api/desktopPreferences.js';
import { normalizeFileOpenBehavior, type FileOpenBehavior } from '#lib/utils/fileOpenBehavior.js';
import { settingsStorage, storage } from '#lib/utils/storage.js';
import {
	beginWallpaperPublication,
	wallpaperPublication
} from '#lib/utils/wallpaperPublication.js';
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

function loadSettings(username: string): UserSettings {
	// Only migrate legacy preferences when the previous sync recorded their owner.
	let stored = settingsStorage.get<StoredUserSettings>(username);
	if (!stored) {
		try {
			if (isLocalDesktop() || storage.getString('boxbox_wallpaper_owner') === username) {
				stored = settingsStorage.get<StoredUserSettings>();
			}
		} catch {
			// Browsers can disable localStorage even when the session is valid.
		}
	}
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
	private desktop: DesktopPreferences | null = null;
	current = $state.raw<UserSettings>({ ...defaultSettings });
	private username: string | null = null;
	private session = 0;
	private sessionController = new AbortController();

	get sessionSignal(): AbortSignal {
		return this.sessionController.signal;
	}
	wallpaperRevision = 0;

	/** Call after sign-in: the drive names come from an authenticated endpoint. */
	async initialize(): Promise<void> {
		this.sessionController.abort();
		this.sessionController = new AbortController();
		const session = ++this.session;
		const username = currentUsername() ?? 'dev';
		this.username = username;
		this.current = loadSettings(username);

		if (isLocalDesktop()) {
			const { DesktopPreferences } = await import('#lib/api/desktopPreferences.js');
			if (session !== this.session) return;
			this.desktop = new DesktopPreferences(this.sessionSignal);
			const saved = await this.desktop.load(defaultSettings, this.current);
			if (session !== this.session) return;
			this.current = saved;
		}
		const driveNameOverrides = await loadDriveNames();
		if (session !== this.session) return;
		this.current = { ...this.current, driveNameOverrides };
		try {
			settingsStorage.set(this.current, username);
		} catch {
			// Initialization should not fail auth because local preference persistence is full.
		}
		if (this.desktop) {
			this.set(this.current);
			await this.flush().catch(() => {});
		}
	}

	flush(): Promise<void> {
		return this.desktop?.flush() ?? Promise.resolve();
	}

	set(settings: UserSettings, wallpaperChanged = false): string | undefined {
		let publication: string | undefined;
		if (!isLocalDesktop() && this.username !== null) {
			if (wallpaperChanged)
				publication = beginWallpaperPublication(this.username, settings.backgroundImage);
			else {
				const intent = wallpaperPublication(this.username);
				if (intent.kind === 'pending' && intent.source === settings.backgroundImage)
					publication = intent.id;
			}
		}
		if (
			settings.backgroundImage !== this.current.backgroundImage ||
			settings.backgroundImageMode !== this.current.backgroundImageMode ||
			settings.frostedGlass !== this.current.frostedGlass
		)
			this.wallpaperRevision++;
		this.current = settings;
		try {
			if (this.username !== null) settingsStorage.set(settings, this.username);
		} catch (error) {
			console.warn('Unable to persist settings', error);
		}
		this.desktop?.save(settings);
		return publication;
	}

	clearSession(): void {
		this.sessionController.abort();
		this.sessionController = new AbortController();
		this.session++;
		this.username = null;
		this.desktop = null;
		this.current = { ...defaultSettings };
	}

	reset(): string | undefined {
		this.wallpaperRevision++;
		return this.set(defaultSettings, true);
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
