import { SvelteSet } from 'svelte/reactivity';
import { isLocalDesktop, openDesktopFile } from '#lib/api/desktop.js';
import {
	createDirectory,
	createFile,
	deleteFile,
	getDirectoryArchiveUrl,
	getDownloadUrl,
	rename,
	type FileInfo
} from '#lib/api/files.js';
import { createDeleteJob } from '#lib/api/jobs.js';
import { useBrowsePaste } from './paste.svelte';
import { clipboardStore } from '#lib/stores/clipboard.svelte.js';
import { jobsStore } from '#lib/stores/jobs.svelte.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { toastStore } from '#lib/stores/toast.svelte.js';
import { canPreview, getFileTypeDescription } from '#lib/utils/fileTypes.js';
import type { BrowseData } from './data.svelte';
import type { BrowseLocation } from './location.svelte';
import type { BrowseSelection } from './selection.svelte';

function errorMessage(error: unknown, fallback: string): string {
	return error instanceof Error ? error.message : fallback;
}

/** Returns the trimmed name, or null (with a toast when the name is unusable). */
function validateItemName(value: string): string | null {
	const name = value.trim();
	if (!name) return null;
	if (name.includes('/') || name.includes('\\')) {
		toastStore.error('Name cannot contain path separators');
		return null;
	}
	return name;
}

function download(items: FileInfo[]) {
	for (const item of items) {
		window.open(
			item.isDir ? getDirectoryArchiveUrl(item.path) : getDownloadUrl(item.path),
			'_blank'
		);
	}
}

export type RenameDialog =
	{ open: true; file: FileInfo; newName: string } | { open: false; file: null; newName: string };

/** What the user does to files and folders: dialogs, file operations, clipboard and preview. */
export function useBrowseActions(
	location: BrowseLocation,
	data: BrowseData,
	selection: BrowseSelection
) {
	const paste = useBrowsePaste(location, data);
	let previewFile = $state<FileInfo | null>(null);
	let openWithFile = $state<FileInfo | null>(null);
	let createDialog = $state<{ open: boolean; type: 'file' | 'directory'; name: string }>({
		open: false,
		type: 'file',
		name: ''
	});
	let renameDialog = $state<RenameDialog>({
		open: false,
		file: null,
		newName: ''
	});
	let deleteDialog = $state<
		| { open: false; items: FileInfo[] }
		| { open: true; items: FileInfo[]; mode: 'trash' | 'permanent' }
	>({ open: false, items: [] });
	let propertiesDialog = $state<{ open: boolean; file: FileInfo | null }>({
		open: false,
		file: null
	});
	// The file or folder being shared; null when the share dialog is closed.
	let shareItem = $state<FileInfo | null>(null);

	const favoritePaths = $derived(
		new SvelteSet(settingsStore.current.favoriteFolders.map((folder) => folder.path))
	);
	const cutPaths = $derived(
		new SvelteSet(
			clipboardStore.operation === 'cut' ? clipboardStore.items.map((item) => item.path) : []
		)
	);

	function closeCreate() {
		createDialog = { open: false, type: 'file', name: '' };
	}

	function openCreate(type: 'file' | 'directory') {
		if (!data.canCreate) {
			toastStore.error('Cannot create items in this location');
			return;
		}
		createDialog = { open: true, type, name: type === 'file' ? 'untitled.txt' : 'New Folder' };
	}

	async function confirmCreate() {
		if (!location.path) return;
		const name = validateItemName(createDialog.name);
		if (!name) return;

		try {
			const created =
				createDialog.type === 'file'
					? await createFile(location.path, name)
					: await createDirectory(location.path, name);
			closeCreate();
			selection.set(new SvelteSet([created.path]));
			toastStore.success(`${created.name} created`);
			data.refresh();
		} catch (error) {
			toastStore.error(errorMessage(error, 'Create failed'));
		}
	}

	async function confirmRename() {
		if (!renameDialog.file) return;
		const newName = validateItemName(renameDialog.newName);
		if (!newName) return;

		const oldPath = renameDialog.file.path;
		const parentPath = oldPath.substring(0, oldPath.lastIndexOf('/'));
		const newPath = parentPath ? `${parentPath}/${newName}` : newName;

		try {
			await rename(oldPath, newPath);
			if (isLocalDesktop()) {
				const { desktopUndo } = await import('./undo.svelte');
				desktopUndo.rememberRename(oldPath, newPath);
			}
			renameDialog = { open: false, file: null, newName: '' };
			data.refetchDirectory();
		} catch (error) {
			console.error('Rename failed:', error);
			toastStore.error(errorMessage(error, 'Rename failed'));
		}
	}

	/** Files go at once; folders become background jobs. */
	async function deleteItems(items: FileInfo[], permanent = !isLocalDesktop()) {
		if (items.length === 0) return;

		try {
			const trashed: string[] = [];
			for (const item of items) {
				if (!permanent) {
					const { trashDesktopFile } = await import('#lib/api/desktopTrash.js');
					const { desktopUndo } = await import('./undo.svelte');
					const result = await trashDesktopFile(item.path);
					trashed.push(result.item.id);
					desktopUndo.rememberTrash([...trashed]);
				} else if (item.isDir) {
					jobsStore.upsertJob(await createDeleteJob(item.path));
				} else {
					await deleteFile(item.path);
				}
			}
			selection.clear();
			data.refetchDirectory();
		} catch (error) {
			console.error('Delete failed:', error);
			toastStore.error(errorMessage(error, 'Delete failed'));
		}
	}

	async function confirmDelete() {
		if (!deleteDialog.open) return;
		await deleteItems(deleteDialog.items, deleteDialog.mode === 'permanent');
		deleteDialog = { open: false, items: [] };
	}

	return {
		paste,
		get openWithFile() {
			return openWithFile;
		},
		closeOpenWith() {
			openWithFile = null;
		},
		get previewFile() {
			return previewFile;
		},
		get createDialog() {
			return createDialog;
		},
		get renameDialog() {
			return renameDialog;
		},
		get deleteDialog() {
			return deleteDialog;
		},
		get propertiesDialog() {
			return propertiesDialog;
		},
		get shareItem() {
			return shareItem;
		},
		get favoritePaths() {
			return favoritePaths;
		},
		get cutPaths() {
			return cutPaths;
		},
		get canPaste() {
			return isLocalDesktop() || clipboardStore.hasItems;
		},

		setCreateName(name: string) {
			createDialog.name = name;
		},
		setRenameName(name: string) {
			renameDialog.newName = name;
		},
		closeCreate,
		closeRename() {
			renameDialog = { open: false, file: null, newName: '' };
		},
		closeDelete() {
			deleteDialog = { open: false, items: [] };
		},
		closeProperties() {
			propertiesDialog = { open: false, file: null };
		},
		closeShare() {
			shareItem = null;
		},
		confirmCreate,
		confirmRename,
		confirmDelete,

		openFile(file: FileInfo) {
			if (file.isDir) {
				location.navigate(file.path);
				return;
			}
			if (isLocalDesktop()) {
				void openDesktopFile(file.path).catch((error: unknown) => {
					toastStore.error(errorMessage(error, 'Open failed'));
				});
				return;
			}
			if (!canPreview(file.name)) {
				toastStore.info(`Preview not available for ${getFileTypeDescription(file.name)}`);
				return;
			}
			previewFile = file;
		},
		showPreview(file: FileInfo) {
			previewFile = file;
		},
		closePreview() {
			previewFile = null;
		},
		fileSaved(file: FileInfo) {
			previewFile = file;
			data.refresh();
		},

		async handleContextMenuAction(action: string, items: FileInfo[]) {
			const [only] = items.length === 1 ? items : [];
			switch (action) {
				case 'open-with':
					if (isLocalDesktop() && only && !only.isDir) openWithFile = only;
					break;
				case 'open-terminal':
					if (isLocalDesktop()) {
						try {
							const { openDesktopTerminal } = await import('#lib/api/desktopApps.js');
							await openDesktopTerminal(only?.isDir ? only.path : location.path);
						} catch (error) {
							toastStore.error(errorMessage(error, 'Could not open a terminal'));
						}
					}
					break;
				case 'new-file':
					openCreate('file');
					break;
				case 'new-folder':
					openCreate('directory');
					break;
				case 'copy':
					clipboardStore.copy(items);
					if (isLocalDesktop()) {
						try {
							const { writeBrowseClipboard } = await import('./desktopClipboard');
							await writeBrowseClipboard(items, 'copy');
						} catch (error) {
							toastStore.error(
								errorMessage(
									error,
									'Native clipboard unavailable; copying within BoxBox still works'
								)
							);
						}
					}
					break;
				case 'cut':
					clipboardStore.cut(items);
					if (isLocalDesktop()) {
						try {
							const { writeBrowseClipboard } = await import('./desktopClipboard');
							await writeBrowseClipboard(items, 'cut');
						} catch (error) {
							toastStore.error(
								errorMessage(
									error,
									'Native clipboard unavailable; moving within BoxBox still works'
								)
							);
						}
					}
					break;
				case 'paste':
					await paste.paste();
					break;
				case 'pin':
					if (only?.isDir) {
						settingsStore.pinFavoriteFolder({ name: only.name, path: only.path });
						toastStore.success(`${only.name} pinned to favorites`);
					}
					break;
				case 'unpin':
					if (only?.isDir) {
						settingsStore.unpinFavoriteFolder(only.path);
						toastStore.success(`${only.name} unpinned from favorites`);
					}
					break;
				case 'rename':
					if (only) renameDialog = { open: true, file: only, newName: only.name };
					break;
				case 'delete':
					if (settingsStore.current.confirmDelete) {
						deleteDialog = { open: true, items, mode: isLocalDesktop() ? 'trash' : 'permanent' };
					} else {
						await deleteItems(items);
					}
					break;
				case 'delete-permanently':
					deleteDialog = { open: true, items, mode: 'permanent' };
					break;
				case 'download':
					download(items);
					break;
				case 'share':
					if (only) shareItem = only;
					break;
				case 'properties':
					if (only) propertiesDialog = { open: true, file: only };
					break;
			}
		}
	};
}

export type BrowseActions = ReturnType<typeof useBrowseActions>;
