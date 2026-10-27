/**
 * The toolbar's "+" menu: everything that puts something new in the current folder.
 */

import type { ContextMenuItem } from '#lib/components/ui/ContextMenu.svelte';
import { FilePlus, FolderPlus, FolderUp, Upload } from 'lucide-svelte';

export type AddAction = 'new-folder' | 'new-file' | 'upload-files' | 'upload-folder';

const ADD_ACTIONS: readonly AddAction[] = [
	'new-folder',
	'new-file',
	'upload-files',
	'upload-folder'
];

export function addMenuItems(canCreate: boolean, canUpload: boolean): ContextMenuItem[] {
	return [
		{ id: 'new-folder', label: 'New folder', icon: FolderPlus, disabled: !canCreate },
		{ id: 'new-file', label: 'New file', icon: FilePlus, disabled: !canCreate },
		{ id: 'separator', label: '', separator: true },
		{ id: 'upload-files', label: 'Upload files', icon: Upload, disabled: !canUpload },
		{ id: 'upload-folder', label: 'Upload folder', icon: FolderUp, disabled: !canUpload }
	];
}

export function toAddAction(id: string): AddAction | undefined {
	return ADD_ACTIONS.find((action) => action === id);
}
