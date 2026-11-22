import { resolve } from '$app/paths';

/**
 * The browse page's address for a folder, as a link to it. The path is always given, even the
 * empty one of Drives, so the page does not take it for a start and open the last location.
 */
export function browseHref(path: string): string {
	return `${resolve('browse')}?${new URLSearchParams({ path })}`;
}

/** Middle-click on a folder opens it in a new browser tab, as a tab does in the desktop app. */
export function openInNewTab(path: string): void {
	window.open(browseHref(path), '_blank', 'noopener');
}

/** The middle button, which opens in a new tab; its press must not start auto-scroll. */
export function isMiddleButton(event: MouseEvent): boolean {
	return event.button === 1;
}
