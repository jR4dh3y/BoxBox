import { resolve } from '$app/paths';

/** The browse page's address for a folder, as a link to it. */
export function browseHref(path: string): string {
	const query = path ? `?${new URLSearchParams({ path })}` : '';
	return `${resolve('browse')}${query}`;
}

/** Middle-click on a folder opens it in a new browser tab, as a tab does in the desktop app. */
export function openInNewTab(path: string): void {
	window.open(browseHref(path), '_blank', 'noopener');
}

/** The middle button, which opens in a new tab; its press must not start auto-scroll. */
export function isMiddleButton(event: MouseEvent): boolean {
	return event.button === 1;
}
