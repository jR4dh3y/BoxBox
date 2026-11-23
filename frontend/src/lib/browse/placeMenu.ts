/**
 * The menu of a place that is not a listed item: a path crumb, a sidebar drive or place, or a
 * favorite, as the desktop app's place menu offers it.
 */

export type Place =
	| { kind: 'folder'; path: string }
	| { kind: 'drive'; path: string }
	// A sidebar place such as Downloads, whose label is its folder's and cannot be renamed.
	| { kind: 'place'; path: string }
	| { kind: 'favorite'; path: string };

export type PlaceAction = 'new-tab' | 'paste' | 'copy-path' | 'rename-drive' | 'unpin';

export interface PlaceMenuItem {
	id: PlaceAction | 'separator';
	label: string;
	disabled?: boolean;
	separator?: boolean;
}

/** The drives page is not a folder: it opens in a tab, but takes no paste. */
export function placeMenuItems(place: Place, canPaste: boolean): PlaceMenuItem[] {
	const folder = place.path !== '';
	const items: PlaceMenuItem[] = [
		{ id: 'new-tab', label: 'Open in New Tab' },
		{ id: 'paste', label: 'Paste Into Folder', disabled: !folder || !canPaste },
		{ id: 'copy-path', label: 'Copy Path', disabled: !folder }
	];
	if (place.kind === 'drive' && folder)
		items.push(
			{ id: 'separator', label: '', separator: true },
			{ id: 'rename-drive', label: 'Rename…' }
		);
	if (place.kind === 'favorite')
		items.push(
			{ id: 'separator', label: '', separator: true },
			{ id: 'unpin', label: 'Remove from Favorites' }
		);
	return items;
}

export function isPlaceAction(id: string): id is PlaceAction {
	return ['new-tab', 'paste', 'copy-path', 'rename-drive', 'unpin'].includes(id);
}
