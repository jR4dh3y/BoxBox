import type { DriveStats } from '#lib/api/files.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';

/** Rename and context-menu state shared by the drive card and the drive list row. */
export function useDriveName(drive: () => DriveStats) {
	let renaming = $state(false);
	let menu = $state<{ x: number; y: number } | null>(null);
	const customName = $derived(settingsStore.current.driveNameOverrides[drive().name]);

	return {
		get displayName() {
			return customName || drive().label || drive().name;
		},
		get hasCustomName() {
			return !!customName;
		},
		get renaming() {
			return renaming;
		},
		get menu() {
			return menu;
		},
		openMenu(event: MouseEvent) {
			event.preventDefault();
			menu = { x: event.clientX, y: event.clientY };
		},
		closeMenu() {
			menu = null;
		},
		selectMenu(id: string) {
			if (id === 'rename') renaming = true;
			else if (id === 'reset') settingsStore.removeDriveName(drive().name);
			menu = null;
		},
		/** An empty name restores the drive's own name. */
		save(value: string) {
			if (value) settingsStore.setDriveName(drive().name, value);
			else settingsStore.removeDriveName(drive().name);
			renaming = false;
		},
		cancel() {
			renaming = false;
		}
	};
}
