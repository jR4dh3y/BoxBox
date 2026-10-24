import type { UserSettings } from '#lib/stores/settings.svelte.js';

/** Refresh untouched wallpaper fields while preserving edits made in Settings. */
export function refreshWallpaperDraft(
	draft: UserSettings,
	previous: UserSettings,
	current: UserSettings
): UserSettings {
	return {
		...draft,
		backgroundImage:
			draft.backgroundImage === previous.backgroundImage
				? current.backgroundImage
				: draft.backgroundImage,
		backgroundImageMode:
			draft.backgroundImageMode === previous.backgroundImageMode
				? current.backgroundImageMode
				: draft.backgroundImageMode,
		frostedGlass:
			draft.frostedGlass === previous.frostedGlass ? current.frostedGlass : draft.frostedGlass
	};
}
