/**
 * File Context Menu Configuration
 * Centralized definition of context menu items for file operations
 */

import type { ContextMenuItem } from '$lib/components/ui/ContextMenu.svelte';
import type { FileInfo } from '$lib/api/files';
import {
	Copy,
	Scissors,
	ClipboardPaste,
	FilePlus,
	FolderPlus,
	Pencil,
	Trash2,
	Download,
	Info,
	Pin,
	PinOff,
	Share2
} from 'lucide-svelte';

export type FileContextAction =
	| 'new-file'
	| 'new-folder'
	| 'copy'
	| 'cut'
	| 'paste'
	| 'pin'
	| 'unpin'
	| 'rename'
	| 'delete'
	| 'download'
	| 'share'
	| 'properties';

export interface FileContextMenuOptions {
	/** Selected items for the context menu */
	items: FileInfo[];
	/** Whether paste is available (clipboard has items) */
	canPaste: boolean;
	/** Paths currently pinned in the sidebar favorites section */
	favoritePaths?: Set<string>;
	/** Whether new files/folders can be created in the current location */
	canCreate?: boolean;
	/** Whether creation actions should be included in this menu */
	includeCreateActions?: boolean;
}

/**
 * Get context menu items for file operations
 * Configures disabled states based on selection
 */
export function getFileContextMenuItems(options: FileContextMenuOptions): ContextMenuItem[] {
	const {
		items,
		canPaste,
		favoritePaths = new Set<string>(),
		canCreate = false,
		includeCreateActions = true
	} = options;
	const hasSelection = items.length > 0;
	const hasMultiple = items.length > 1;
	const hasFolder = items.some((i) => i.isDir);
	const singleFile = items.length === 1 && !items[0].isDir;
	const singleFolder = !hasMultiple && items[0]?.isDir ? items[0] : null;
	const isFavorite = singleFolder ? favoritePaths.has(singleFolder.path) : false;
	const createItems: ContextMenuItem[] = includeCreateActions
		? [
				{ id: 'new-file', label: 'New File', icon: FilePlus, disabled: !canCreate },
				{ id: 'new-folder', label: 'New Folder', icon: FolderPlus, disabled: !canCreate },
				{ id: 'separator-create', label: '', separator: true }
			]
		: [];

	if (!hasSelection) {
		return [
			...createItems,
			{ id: 'paste', label: 'Paste', icon: ClipboardPaste, shortcut: 'Ctrl+V', disabled: !canPaste }
		];
	}

	return [
		...createItems,
		{ id: 'copy', label: 'Copy', icon: Copy, shortcut: 'Ctrl+C' },
		{ id: 'cut', label: 'Cut', icon: Scissors, shortcut: 'Ctrl+X' },
		{ id: 'paste', label: 'Paste', icon: ClipboardPaste, shortcut: 'Ctrl+V', disabled: !canPaste },
		...(singleFolder
			? [
					{
						id: isFavorite ? 'unpin' : 'pin',
						label: isFavorite ? 'Unpin from Favorites' : 'Pin to Favorites',
						icon: isFavorite ? PinOff : Pin
					}
				]
			: []),
		{ id: 'separator-1', label: '', separator: true },
		{ id: 'rename', label: 'Rename', icon: Pencil, shortcut: 'F2', disabled: hasMultiple },
		{ id: 'delete', label: 'Delete', icon: Trash2, shortcut: 'Del' },
		{ id: 'separator-2', label: '', separator: true },
		{ id: 'download', label: 'Download', icon: Download, disabled: hasFolder },
		{ id: 'share', label: 'Share…', icon: Share2, disabled: !singleFile },
		{ id: 'properties', label: 'Properties', icon: Info, disabled: hasMultiple }
	];
}
