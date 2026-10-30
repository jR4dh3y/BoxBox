export type SettingsSection = 'general' | 'appearance' | 'links' | 'account' | 'connections';

export interface SettingsSectionEntry {
	id: SettingsSection;
	label: string;
}

/**
 * The local desktop has no sign-in or pairing, so Account belongs to the server web UI.
 * Connections manage saved deployments, which only the desktop host holds.
 */
export function settingsSections(localDesktop: boolean): SettingsSectionEntry[] {
	return [
		{ id: 'general', label: 'General' },
		{ id: 'appearance', label: 'Appearance' },
		{ id: 'links', label: 'Shared links' },
		localDesktop ? { id: 'connections', label: 'Connections' } : { id: 'account', label: 'Account' }
	];
}

/** A request for a section this runtime does not show opens General instead of an orphan panel. */
export function initialSettingsSection(
	sections: readonly SettingsSectionEntry[],
	requested: SettingsSection
): SettingsSection {
	return sections.some((section) => section.id === requested) ? requested : 'general';
}
