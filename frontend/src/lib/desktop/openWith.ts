import type { DesktopApplication, DesktopAppsResponse } from '#lib/api/desktopApps.js';

export interface OpenWithChoices {
	recommended: DesktopApplication[];
	other: DesktopApplication[];
}

function matches(app: DesktopApplication, query: string): boolean {
	return app.name.toLocaleLowerCase().includes(query) || app.id.toLowerCase().includes(query);
}

/** Search covers both groups so an app outside the file type's associations is one query away. */
export function filterOpenWithChoices(apps: DesktopAppsResponse, search: string): OpenWithChoices {
	const query = search.trim().toLocaleLowerCase();
	if (!query) return { recommended: apps.recommended, other: apps.other };
	return {
		recommended: apps.recommended.filter((app) => matches(app, query)),
		other: apps.other.filter((app) => matches(app, query))
	};
}

/** The default handler, else the first recommended app. Nothing is preselected from other apps. */
export function initialOpenWithChoice(apps: DesktopAppsResponse): string {
	return (apps.recommended.find((app) => app.isDefault) ?? apps.recommended[0])?.id ?? '';
}

/** A choice hidden by the search cannot be opened by mistake. */
export function visibleOpenWithChoice(choices: OpenWithChoices, selected: string): string {
	return [...choices.recommended, ...choices.other].some((app) => app.id === selected)
		? selected
		: '';
}
