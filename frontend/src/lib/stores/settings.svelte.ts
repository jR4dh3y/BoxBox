/**
 * User preferences, kept in localStorage; drive names are also stored on the server
 */

import { isLocalDesktop } from '#lib/api/desktop.js';
import { DEFAULT_SORT_BY, DEFAULT_SORT_DIR } from '#lib/browse/sortFiles.js';
import {
	isRootPath,
	parseFolderViews,
	rememberFolderMode,
	rememberFolderSort,
	type FolderSort,
	type FolderViews
} from '#lib/browse/folderViews.js';
import type { ViewMode } from '#lib/types/files.js';
import {
	DEFAULT_ZOOM,
	GRID_STEPS,
	LIST_STEPS,
	savedZoom,
	zoomStep,
	type ZoomChange
} from '#lib/browse/zoom.js';
import { parseColumnLayout, type ColumnLayout } from '#lib/browse/columns.js';
import {
	parseFolderLooks,
	withFolderLook,
	type FolderLook,
	type FolderLooks
} from '#lib/browse/folderLooks.js';
import type { DesktopPreferences } from '#lib/api/desktopPreferences.js';
import { settingsStorage, storage } from '#lib/utils/storage.js';
import {
	beginWallpaperPublication,
	wallpaperPublication
} from '#lib/utils/wallpaperPublication.js';
import { currentUsername } from '#lib/utils/sessionUser.js';
import { normalizeAccentColorSource, type AccentColorSource } from '#lib/utils/appearance.js';
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
	/** Off: List or Grid picked in any folder is the view of every folder. */
	viewPerFolder: boolean;
	/** Each folder's view and sort, where they differ from the defaults above. */
	folderViews: FolderViews;
	/** The list's columns as shown and sized by hand; null keeps the defaults. */
	fileColumns: ColumnLayout | null;
	/** Folders given their own colour or emblem, by browse path. */
	folderLooks: FolderLooks;
	accentColor: string | null;
	accentColorSource: AccentColorSource;
	backgroundImage: string | null;
	/** The wallpaper's file name, when it is known; a restored upload from another browser has none. */
	backgroundImageName: string | null;
	backgroundImageMode: BackgroundImageMode;
	frostedGlass: boolean;
	wallpaperTint: boolean;
	hardwareAcceleration: boolean;
	previewOnSingleClick: boolean;
	/** Zoom steps of the list and the grid; see `browse/zoom.ts`. */
	listZoom: number;
	gridZoom: number;
	/** Grid tiles draw a few of what a folder holds inside it. */
	folderPreviews: boolean;
	/** A card with an item's details shows when the pointer rests on it. */
	hoverDetails: boolean;
	/** Rows show a checkbox on hover, so several items can be picked without modifier keys. */
	selectionCheckboxes: boolean;
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
	defaultSortBy: DEFAULT_SORT_BY,
	defaultSortDir: DEFAULT_SORT_DIR,
	defaultViewMode: 'list',
	viewPerFolder: true,
	folderViews: {},
	fileColumns: null,
	folderLooks: {},
	accentColor: null,
	accentColorSource: 'wallpaper',
	backgroundImage: null,
	backgroundImageName: null,
	backgroundImageMode: DEFAULT_BACKGROUND_IMAGE_MODE,
	frostedGlass: false,
	wallpaperTint: false,
	hardwareAcceleration: true,
	previewOnSingleClick: false,
	listZoom: DEFAULT_ZOOM,
	gridZoom: DEFAULT_ZOOM,
	hoverDetails: true,
	folderPreviews: true,
	selectionCheckboxes: true,
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
		accentColorSource: normalizeAccentColorSource(stored?.accentColorSource, settings.accentColor),
		folderViews: parseFolderViews(settings.folderViews),
		listZoom: savedZoom(stored, 'listZoom'),
		gridZoom: savedZoom(stored, 'gridZoom'),
		fileColumns: parseColumnLayout(settings.fileColumns),
		folderLooks: parseFolderLooks(settings.folderLooks),
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
	private desktop: DesktopPreferences | undefined;
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
			const loaded = await this.desktop.load(defaultSettings, this.current);
			if (session !== this.session) return;
			this.current = loaded;
		}

		const driveNameOverrides = await loadDriveNames();
		if (session !== this.session) return;
		this.current = { ...this.current, driveNameOverrides };
		try {
			settingsStorage.set(this.current, username);
		} catch {
			// Initialization should not fail auth because local preference persistence is full.
		}
	}

	set(settings: UserSettings, wallpaperChanged = false): string | undefined {
		let publication: string | undefined;
		if (this.username !== null) {
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
		this.desktop?.save(settings);
		try {
			if (this.username !== null) settingsStorage.set(settings, this.username);
		} catch (error) {
			console.warn('Unable to persist settings', error);
		}
		return publication;
	}

	clearSession(): void {
		this.sessionController.abort();
		this.sessionController = new AbortController();
		this.session++;
		this.desktop = undefined;
		this.username = null;
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

	/**
	 * The view picked in a folder, or in every folder when views are not kept per folder.
	 * The drives keep theirs either way; their default is the cards.
	 */
	chooseFolderMode(path: string, mode: ViewMode): void {
		const settings = this.current;
		const root = isRootPath(path);
		if (!root && !settings.viewPerFolder) {
			this.set({ ...settings, defaultViewMode: mode });
			return;
		}
		const folderViews = rememberFolderMode(
			settings.folderViews,
			path,
			mode,
			root ? 'grid' : settings.defaultViewMode
		);
		this.set({ ...settings, folderViews });
	}

	chooseFolderSort(path: string, sort: FolderSort): void {
		const settings = this.current;
		const folderViews = rememberFolderSort(settings.folderViews, path, sort, {
			by: settings.defaultSortBy,
			dir: settings.defaultSortDir
		});
		this.set({ ...settings, folderViews });
	}

	/** Steps the zoom of the list or the grid, for every folder. Returns whether it changed. */
	zoom(view: ViewMode, change: ZoomChange): boolean {
		const settings = this.current;
		const key = view === 'list' ? 'listZoom' : 'gridZoom';
		const count = view === 'list' ? LIST_STEPS.length : GRID_STEPS.length;
		const next = zoomStep(settings[key], change, count);
		if (next === settings[key]) return false;
		this.set({ ...settings, [key]: next });
		return true;
	}

	setFolderLook(path: string, look: FolderLook): void {
		this.set({
			...this.current,
			folderLooks: withFolderLook(this.current.folderLooks, path, look)
		});
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
