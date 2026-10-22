import type { ContextMenuItem } from '#lib/components/ui/ContextMenu.svelte';
import type { FileInfo } from '#lib/api/files.js';
import { isLocalDesktop } from '#lib/api/desktop.js';
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
	Share2,
	AppWindow,
	PanelTop,
	Terminal
} from 'lucide-svelte';

export type FileContextAction =
	| 'open-in-new-tab'
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
	| 'properties'
	| 'open-with'
	| 'open-terminal'
	| 'delete-permanently';

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

type FileContextMenuItem = ContextMenuItem<FileContextAction>;

/** Menu entries for the clicked items, or for the folder background when `items` is empty. */
export function getFileContextMenuItems(options: FileContextMenuOptions): FileContextMenuItem[] {
	const {
		items,
		canPaste,
		favoritePaths = new Set<string>(),
		canCreate = false,
		includeCreateActions = true
	} = options;
	const desktop = isLocalDesktop();
	const hasMultiple = items.length > 1;
	const singleFile = items.length === 1 && !items[0].isDir;
	const singleFolder = !hasMultiple && items[0]?.isDir ? items[0] : null;
	const paste: FileContextMenuItem = {
		id: 'paste',
		label: 'Paste',
		icon: ClipboardPaste,
		shortcut: 'Ctrl+V',
		disabled: !canPaste
	};
	const menu: FileContextMenuItem[] = [];

	if (includeCreateActions) {
		menu.push(
			{ id: 'new-file', label: 'New File', icon: FilePlus, disabled: !canCreate },
			{ id: 'new-folder', label: 'New Folder', icon: FolderPlus, disabled: !canCreate },
			'separator'
		);
	}

	if (items.length === 0) {
		if (desktop) menu.push({ id: 'open-terminal', label: 'Open Terminal Here', icon: Terminal });
		menu.push(paste);
		return menu;
	}

	// Only actions that apply to this selection; a greyed-out entry never becomes usable here.
	if (singleFolder) {
		menu.push({ id: 'open-in-new-tab', label: 'Open in New Tab', icon: PanelTop });
		if (desktop) menu.push({ id: 'open-terminal', label: 'Open Terminal Here', icon: Terminal });
		menu.push('separator');
	} else if (desktop && singleFile) {
		menu.push({ id: 'open-with', label: 'Open With…', icon: AppWindow }, 'separator');
	}
	menu.push(
		{ id: 'copy', label: 'Copy', icon: Copy, shortcut: 'Ctrl+C' },
		{ id: 'cut', label: 'Cut', icon: Scissors, shortcut: 'Ctrl+X' },
		paste
	);
	if (singleFolder) {
		const pinned = favoritePaths.has(singleFolder.path);
		menu.push(
			pinned
				? { id: 'unpin', label: 'Unpin from Favorites', icon: PinOff }
				: { id: 'pin', label: 'Pin to Favorites', icon: Pin }
		);
	}
	menu.push(
		'separator',
		{ id: 'rename', label: 'Rename', icon: Pencil, shortcut: 'F2', disabled: hasMultiple },
		{ id: 'delete', label: desktop ? 'Move to Trash' : 'Delete', icon: Trash2, shortcut: 'Del' }
	);
	if (desktop) {
		menu.push({
			id: 'delete-permanently',
			label: 'Delete Permanently…',
			icon: Trash2,
			shortcut: 'Shift+Del'
		});
	}
	menu.push(
		'separator',
		{
			id: 'download',
			label: singleFolder ? 'Download as ZIP' : 'Download',
			icon: Download,
			disabled: items.some((item) => item.isDir) && !singleFolder
		},
		{
			id: 'share',
			label: singleFile ? 'Share file…' : 'Share folder…',
			icon: Share2,
			disabled: items.length !== 1
		},
		{ id: 'properties', label: 'Properties', icon: Info, disabled: hasMultiple }
	);
	return menu;
}
