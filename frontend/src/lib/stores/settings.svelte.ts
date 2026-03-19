/**
 * Settings store using Svelte 5 runes.
 */

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
	driveNameOverrides: {}
};

function cloneSettings(settings: UserSettings): UserSettings {
	return {
		...settings,
		driveNameOverrides: { ...settings.driveNameOverrides }
	};
}

function loadSettings(): UserSettings {
	const stored = settingsStorage.get<UserSettings>();
	if (!stored) {
		return cloneSettings(defaultSettings);
	}

	return {
		...defaultSettings,
		...stored,
		driveNameOverrides: {
			...defaultSettings.driveNameOverrides,
			...(stored.driveNameOverrides ?? {})
		}
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

class SettingsStore {
	private state = $state<UserSettings>(loadSettings());

	get value(): UserSettings {
		return this.state;
	}

	get showHiddenFiles(): boolean {
		return this.state.showHiddenFiles;
	}

	get showFileExtensions(): boolean {
		return this.state.showFileExtensions;
	}

	get confirmDelete(): boolean {
		return this.state.confirmDelete;
	}

	get compactMode(): boolean {
		return this.state.compactMode;
	}

	get driveNameOverrides(): Record<string, string> {
		return this.state.driveNameOverrides;
	}

	async initialize(): Promise<void> {
		const driveNames = await loadDriveNames();
		this.state.driveNameOverrides = driveNames;
		saveSettings(this.state);
	}

	set(settings: UserSettings): void {
		this.state = cloneSettings(settings);
		saveSettings(this.state);
	}

	update(updater: (settings: UserSettings) => UserSettings): void {
		this.state = cloneSettings(updater(cloneSettings(this.state)));
		saveSettings(this.state);
	}

	reset(): void {
		this.state = cloneSettings(defaultSettings);
		saveSettings(this.state);
	}

	setSetting<K extends keyof UserSettings>(key: K, value: UserSettings[K]): void {
		this.state[key] = value;
		saveSettings(this.state);
	}

	getSetting<K extends keyof UserSettings>(key: K): UserSettings[K] {
		return this.state[key];
	}

	async setDriveName(originalName: string, customName: string): Promise<void> {
		await apiSetDriveName({ mountPoint: originalName, customName });
		this.state.driveNameOverrides = {
			...this.state.driveNameOverrides,
			[originalName]: customName
		};
		saveSettings(this.state);
	}

	async removeDriveName(originalName: string): Promise<void> {
		await apiDeleteDriveName(originalName);
		const rest = { ...this.state.driveNameOverrides };
		delete rest[originalName];
		this.state.driveNameOverrides = rest;
		saveSettings(this.state);
	}

	getDriveName(originalName: string): string | null {
		return this.state.driveNameOverrides[originalName] ?? null;
	}
}

export const settingsStore = new SettingsStore();
