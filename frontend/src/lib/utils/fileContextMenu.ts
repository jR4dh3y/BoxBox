/**
 * File Context Menu Configuration
 * Centralized definition of context menu items for file operations
 */

import type { ContextMenuItem } from '$lib/components/ui/ContextMenu.svelte';
import type { FileInfo } from '$lib/api/files';
import { Copy, Scissors, ClipboardPaste, Pencil, Trash2, Download, Info } from 'lucide-svelte';

export type FileContextAction =
	| 'copy'
	| 'cut'
	| 'paste'
	| 'rename'
	| 'delete'
	| 'download'
	| 'properties';

export interface FileContextMenuOptions {
	/** Selected items for the context menu */
	items: FileInfo[];
	/** Whether paste is available (clipboard has items) */
	canPaste: boolean;
}

/**
 * Get context menu items for file operations
 * Configures disabled states based on selection
 */
export function getFileContextMenuItems(options: FileContextMenuOptions): ContextMenuItem[] {
	const { items, canPaste } = options;
	const hasMultiple = items.length > 1;
	const hasFolder = items.some((i) => i.isDir);

	return [
		{ id: 'copy', label: 'Copy', icon: Copy, shortcut: 'Ctrl+C' },
		{ id: 'cut', label: 'Cut', icon: Scissors, shortcut: 'Ctrl+X' },
		{ id: 'paste', label: 'Paste', icon: ClipboardPaste, shortcut: 'Ctrl+V', disabled: !canPaste },
		{ id: 'separator-1', label: '', separator: true },
		{ id: 'rename', label: 'Rename', icon: Pencil, shortcut: 'F2', disabled: hasMultiple },
		{ id: 'delete', label: 'Delete', icon: Trash2, shortcut: 'Del' },
		{ id: 'separator-2', label: '', separator: true },
		{ id: 'download', label: 'Download', icon: Download, disabled: hasFolder },
		{ id: 'properties', label: 'Properties', icon: Info, disabled: hasMultiple }
	];
}
