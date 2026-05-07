/**
 * Settings store - persists user preferences to localStorage and API
 */
import { writable, derived, get } from 'svelte/store';
import { settingsStorage } from '$lib/utils/storage';
import {
	getDriveNames,
	setDriveName as apiSetDriveName,
	deleteDriveName as apiDeleteDriveName
} from '$lib/api/drive-names';

export interface UserSettings {
	showHiddenFiles: boolean;
	showFileExtensions: boolean;
	confirmDelete: boolean;
	defaultSortBy: 'name' | 'size' | 'modTime' | 'type';
	defaultSortDir: 'asc' | 'desc';
	defaultViewMode: 'list' | 'grid';
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
	previewOnSingleClick: false,
	compactMode: false,
	driveNameOverrides: {},
	favoriteFolders: []
};

function loadSettings(): UserSettings {
	const stored = settingsStorage.get<UserSettings>();
	return stored ? { ...defaultSettings, ...stored } : defaultSettings;
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

function createSettingsStore() {
	const { subscribe, set, update } = writable<UserSettings>(loadSettings());

	return {
		subscribe,

		async initialize() {
			const driveNames = await loadDriveNames();
			update((current) => ({ ...current, driveNameOverrides: driveNames }));
		},

		set(settings: UserSettings) {
			saveSettings(settings);
			set(settings);
		},

		update(updater: (settings: UserSettings) => UserSettings) {
			update((current) => {
				const updated = updater(current);
				saveSettings(updated);
				return updated;
			});
		},

		reset() {
			saveSettings(defaultSettings);
			set(defaultSettings);
		},

		setSetting<K extends keyof UserSettings>(key: K, value: UserSettings[K]) {
			update((current) => {
				const updated = { ...current, [key]: value };
				saveSettings(updated);
				return updated;
			});
		},

		getSetting<K extends keyof UserSettings>(key: K): UserSettings[K] {
			return get({ subscribe })[key];
		},

		async setDriveName(originalName: string, customName: string) {
			await apiSetDriveName({ mountPoint: originalName, customName });
			update((current) => {
				const updated = {
					...current,
					driveNameOverrides: { ...current.driveNameOverrides, [originalName]: customName }
				};
				saveSettings(updated);
				return updated;
			});
		},

		async removeDriveName(originalName: string) {
			await apiDeleteDriveName(originalName);
			update((current) => {
				const { [originalName]: removed, ...rest } = current.driveNameOverrides;
				const updated = { ...current, driveNameOverrides: rest };
				saveSettings(updated);
				return updated;
			});
		},

		getDriveName(originalName: string): string | null {
			return get({ subscribe }).driveNameOverrides[originalName] || null;
		},

		get driveNameOverrides(): Record<string, string> {
			return get({ subscribe }).driveNameOverrides;
		},

		pinFavoriteFolder(folder: FavoriteFolder) {
			update((current) => {
				if (current.favoriteFolders.some((favorite) => favorite.path === folder.path)) {
					return current;
				}

				const updated = {
					...current,
					favoriteFolders: [...current.favoriteFolders, folder]
				};
				saveSettings(updated);
				return updated;
			});
		},

		unpinFavoriteFolder(path: string) {
			update((current) => {
				const updated = {
					...current,
					favoriteFolders: current.favoriteFolders.filter((folder) => folder.path !== path)
				};
				saveSettings(updated);
				return updated;
			});
		},

		isFavoriteFolder(path: string): boolean {
			return get({ subscribe }).favoriteFolders.some((folder) => folder.path === path);
		}
	};
}

export const settingsStore = createSettingsStore();
// Note: initialize() should be called after successful authentication
// Do not call here as the API requires auth

// Derived stores for individual settings
export const showHiddenFiles = derived(settingsStore, ($s) => $s.showHiddenFiles);
export const showFileExtensions = derived(settingsStore, ($s) => $s.showFileExtensions);
export const confirmDelete = derived(settingsStore, ($s) => $s.confirmDelete);
export const compactMode = derived(settingsStore, ($s) => $s.compactMode);
