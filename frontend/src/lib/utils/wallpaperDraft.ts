import type { UserSettings } from '#lib/stores/settings.svelte.js';

/** Refresh untouched wallpaper fields while preserving edits made in Settings. */
export function refreshWallpaperDraft(
	draft: UserSettings,
	previous: UserSettings,
	current: UserSettings
): UserSettings {
	// A replacement image and its display choices are one explicit edit.
	if (draft.backgroundImage !== previous.backgroundImage) return draft;

	return {
		...draft,
		backgroundImage: current.backgroundImage,
		backgroundImageMode:
			draft.backgroundImageMode === previous.backgroundImageMode
				? current.backgroundImageMode
				: draft.backgroundImageMode,
		frostedGlass:
			draft.frostedGlass === previous.frostedGlass ? current.frostedGlass : draft.frostedGlass
	};
}
