/**
 * File Context Menu Configuration
 * Centralized definition of context menu items for file operations
 */

import { isNativePath, supportsServerTransfer } from '#lib/api/desktop.js';
import type { ContextMenuItem } from '#lib/components/ui/ContextMenu.svelte';
import type { FileInfo } from '#lib/api/files.js';
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
	| 'preview'
	| 'open-with'
	| 'open-terminal'
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
	/** Whether the current source refuses file mutations. */
	readOnly?: boolean;
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
		readOnly = false,
		includeCreateActions = true
	} = options;
	const hasSelection = items.length > 0;
	const nativeSelection = hasSelection && items.every((item) => isNativePath(item.path));
	const hasMultiple = items.length > 1;
	const hasFolder = items.some((i) => i.isDir);
	const singleFile = items.length === 1 && !items[0].isDir;
	const singleItem = items.length === 1;
	const singleFolder = !hasMultiple && items[0]?.isDir ? items[0] : null;
	const isFavorite = singleFolder ? favoritePaths.has(singleFolder.path) : false;
	const createItems: ContextMenuItem[] = includeCreateActions
		? [
				{ id: 'new-file', label: 'New File', icon: FilePlus, disabled: readOnly || !canCreate },
				{
					id: 'new-folder',
					label: 'New Folder',
					icon: FolderPlus,
					disabled: readOnly || !canCreate
				},
				{ id: 'separator-create', label: '', separator: true }
			]
		: [];

	if (!hasSelection) {
		return [
			...createItems,
			{
				id: 'paste',
				label: 'Paste',
				icon: ClipboardPaste,
				shortcut: 'Ctrl+V',
				disabled: readOnly || !canPaste
			}
		];
	}

	return [
		...createItems,
		...(singleItem && isNativePath(items[0].path)
			? singleFolder
				? [{ id: 'open-terminal', label: 'Open in Terminal', icon: Info }]
				: [
						{ id: 'open-with', label: 'Open with…', icon: Info },
						{ id: 'preview', label: 'Preview', icon: Info }
					]
			: []),
		{ id: 'copy', label: 'Copy', icon: Copy, shortcut: 'Ctrl+C' },
		{ id: 'cut', label: 'Cut', icon: Scissors, shortcut: 'Ctrl+X', disabled: readOnly },
		{
			id: 'paste',
			label: 'Paste',
			icon: ClipboardPaste,
			shortcut: 'Ctrl+V',
			disabled: readOnly || !canPaste
		},
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
		{
			id: 'rename',
			label: 'Rename',
			icon: Pencil,
			shortcut: 'F2',
			disabled: readOnly || hasMultiple
		},
		{
			id: 'delete',
			label: nativeSelection ? 'Move to Trash' : 'Delete',
			icon: Trash2,
			shortcut: 'Del',
			disabled: readOnly
		},
		{ id: 'separator-2', label: '', separator: true },
		...(supportsServerTransfer(items)
			? [
					{
						id: 'download',
						label: singleFolder ? 'Download as ZIP' : 'Download',
						icon: Download,
						disabled: hasFolder && !singleFolder
					},
					{
						id: 'share',
						label: singleFile ? 'Share file…' : 'Share folder…',
						icon: Share2,
						disabled: !singleItem
					}
				]
			: []),
		{ id: 'properties', label: 'Properties', icon: Info, disabled: hasMultiple }
	];
}
