import { SvelteSet } from 'svelte/reactivity';
import { isNativePath, openDesktopFile } from '#lib/api/desktop.js';
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
import type { FileContextAction } from '#lib/utils/fileContextMenu.js';
import { useBrowsePaste } from './paste.svelte';
import {
	beginBrowseClipboardPublication,
	finishBrowseClipboardPublication,
	publishBrowseClipboardInOrder
} from './clipboardPublication';
import { clipboardStore } from '#lib/stores/clipboard.svelte.js';
import { jobsStore } from '#lib/stores/jobs.svelte.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { toastStore } from '#lib/stores/toast.svelte.js';
import { canPreview, getFileTypeDescription } from '#lib/utils/fileTypes.js';
import type { BrowseData } from './data.svelte';
import type { BrowseLocation } from './location.svelte';
import type { BrowseSelection } from './selection.svelte';

export type BrowseOperation = 'creating' | 'renaming' | 'deleting' | null;

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

/** What the user does to files and folders: dialogs, file operations, clipboard and preview. */
export function useBrowseActions(
	location: BrowseLocation,
	data: BrowseData,
	selection: BrowseSelection
) {
	const paste = useBrowsePaste(location, data);
	let operation = $state<BrowseOperation>(null);
	let previewFile = $state<FileInfo | null>(null);
	let openWithFile = $state<FileInfo | null>(null);
	let createDialog = $state<{ open: boolean; type: 'file' | 'directory'; name: string }>({
		open: false,
		type: 'file',
		name: ''
	});
	let renameDialog = $state<{ open: boolean; file: FileInfo | null; newName: string }>({
		open: false,
		file: null,
		newName: ''
	});
	let deleteDialog = $state<
		| { open: false; items: FileInfo[] }
		| { open: true; items: FileInfo[]; mode: 'trash' | 'permanent' }
	>({ open: false, items: [] });
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
		if (operation !== null || !location.path) return;
		const name = validateItemName(createDialog.name);
		if (!name) return;

		operation = 'creating';
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
		} finally {
			operation = null;
		}
	}

	async function confirmRename() {
		if (operation !== null || !renameDialog.file) return;
		const file = renameDialog.file;
		const newName = validateItemName(renameDialog.newName);
		if (!newName) return;

		const oldPath = file.path;
		const parentPath = oldPath.substring(0, oldPath.lastIndexOf('/'));
		const newPath = parentPath ? `${parentPath}/${newName}` : newName;

		operation = 'renaming';
		try {
			await rename(oldPath, newPath);
			if (isNativePath(oldPath)) {
				const { desktopUndo } = await import('./undo.svelte');
				desktopUndo.rememberRename(oldPath, newPath);
			}
			if (
				renameDialog.open &&
				renameDialog.file?.path === oldPath &&
				renameDialog.newName === newName
			) {
				renameDialog = { open: false, file: null, newName: '' };
			}
			toastStore.success(`${file.name} renamed to ${newName}`);
			data.refetchDirectory();
		} catch (error) {
			toastStore.error(errorMessage(error, 'Rename failed'));
		} finally {
			operation = null;
		}
	}

	/** Files go at once; folders become background jobs. */
	async function deleteItems(
		items: FileInfo[],
		permanent = !isNativePath(location.path)
	): Promise<boolean> {
		if (operation !== null || items.length === 0) return false;
		operation = 'deleting';
		const completed: FileInfo[] = [];
		const failed: FileInfo[] = [];
		let lastError: unknown;
		try {
			let trash:
				[typeof import('#lib/api/desktopTrash.js'), typeof import('./undo.svelte')] | null = null;
			if (!permanent) {
				try {
					trash = await Promise.all([import('#lib/api/desktopTrash.js'), import('./undo.svelte')]);
				} catch {
					// A failed chunk stays failed until the page reloads.
					toastStore.error('Trash could not load. Reload BoxBox and try again.');
					return false;
				}
			}
			const trashed: string[] = [];
			for (const item of items) {
				try {
					if (trash) {
						const [{ trashDesktopFile }, { desktopUndo }] = trash;
						trashed.push((await trashDesktopFile(item.path)).id);
						desktopUndo.rememberTrash([...trashed]);
					} else if (item.isDir) {
						jobsStore.upsertJob(await createDeleteJob(item.path));
					} else {
						await deleteFile(item.path);
					}
					completed.push(item);
				} catch (error) {
					failed.push(item);
					lastError = error;
				}
			}

			if (completed.length > 0) {
				const completedPaths = new SvelteSet(completed.map((item) => item.path));
				selection.set(
					new SvelteSet([...selection.paths].filter((path) => !completedPaths.has(path)))
				);
				data.refetchDirectory();
			}
			if (failed.length > 0) {
				if (deleteDialog.open && deleteDialog.items === items) {
					deleteDialog = { ...deleteDialog, items: failed };
				}
				const progress = completed.length > 0 ? `${completed.length} completed; ` : '';
				toastStore.error(
					`${progress}${failed.length} could not be deleted: ${errorMessage(lastError, 'Delete failed')}`
				);
				return false;
			}

			const count = completed.length;
			if (!permanent) {
				toastStore.success(`Moved ${count} ${count === 1 ? 'item' : 'items'} to Trash`);
			} else if (completed.some((item) => item.isDir)) {
				toastStore.success(`Delete started for ${count} ${count === 1 ? 'item' : 'items'}`);
			} else {
				toastStore.success(`Deleted ${count} ${count === 1 ? 'item' : 'items'}`);
			}
			return true;
		} finally {
			operation = null;
		}
	}

	async function confirmDelete() {
		if (!deleteDialog.open) return;
		const dialog = deleteDialog;
		const completed = await deleteItems(dialog.items, dialog.mode === 'permanent');
		if (completed && deleteDialog.open && deleteDialog.items === dialog.items) {
			deleteDialog = { open: false, items: [] };
		}
	}

	async function setClipboard(operation: 'copy' | 'cut', items: FileInfo[]) {
		clipboardStore[operation](items);
		const selectionId = beginBrowseClipboardPublication();
		if (!items.every((item) => isNativePath(item.path))) {
			finishBrowseClipboardPublication(selectionId, 'failed');
			return;
		}
		try {
			await publishBrowseClipboardInOrder(async () => {
				const { setDesktopClipboard } = await import('#lib/api/desktopClipboard.js');
				await setDesktopClipboard(
					items.map((item) => item.path),
					operation
				);
			});
			finishBrowseClipboardPublication(selectionId, 'published');
		} catch (error) {
			finishBrowseClipboardPublication(selectionId, 'failed');
			toastStore.error(
				errorMessage(error, 'Native clipboard unavailable; BoxBox can still paste within the app')
			);
		}
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
			return isNativePath(location.path) || clipboardStore.hasItems;
		},
		get operation() {
			return operation;
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
			if (isNativePath(file.path)) {
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

		async handleContextMenuAction(action: FileContextAction, items: FileInfo[]) {
			const [only] = items.length === 1 ? items : [];
			switch (action) {
				case 'open-with':
					if (isNativePath(location.path) && only && !only.isDir) openWithFile = only;
					break;
				case 'open-terminal':
					if (isNativePath(location.path)) {
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
				case 'cut':
					await setClipboard(action, items);
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
						deleteDialog = {
							open: true,
							items,
							mode: isNativePath(location.path) ? 'trash' : 'permanent'
						};
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
			}
		}
	};
}

export type BrowseActions = ReturnType<typeof useBrowseActions>;
