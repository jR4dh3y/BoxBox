import { SvelteSet } from 'svelte/reactivity';
import {
	createDirectory,
	createFile,
	deleteFile,
	getDirectoryArchiveUrl,
	getDownloadUrl,
	rename,
	type FileInfo
} from '$lib/api/files';
import { createCopyJob, createDeleteJob, createMoveJob } from '$lib/api/jobs';
import { clipboardStore } from '$lib/stores/clipboard.svelte';
import { jobsStore } from '$lib/stores/jobs.svelte';
import { settingsStore } from '$lib/stores/settings.svelte';
import { toastStore } from '$lib/stores/toast.svelte';
import { canPreview, getFileTypeDescription } from '$lib/utils/fileTypes';
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

/** What the user does to files and folders: dialogs, file operations, clipboard and preview. */
export function useBrowseActions(
	location: BrowseLocation,
	data: BrowseData,
	selection: BrowseSelection
) {
	let previewFile = $state<FileInfo | null>(null);
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
	let deleteDialog = $state<{ open: boolean; items: FileInfo[] }>({ open: false, items: [] });
	let propertiesDialog = $state<{ open: boolean; file: FileInfo | null }>({
		open: false,
		file: null
	});
	let shareDialog = $state<{ open: boolean; file: FileInfo | null }>({ open: false, file: null });
	let folderShareDialog = $state<{ open: boolean; folder: FileInfo | null }>({
		open: false,
		folder: null
	});

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

	async function paste() {
		if (!clipboardStore.hasItems || !location.path) return;

		const operation = clipboardStore.operation;
		try {
			for (const item of clipboardStore.items) {
				const destPath = `${location.path}/${item.name}`;
				if (operation === 'copy') {
					jobsStore.upsertJob(await createCopyJob(item.path, destPath));
				} else if (operation === 'cut') {
					jobsStore.upsertJob(await createMoveJob(item.path, destPath));
				}
			}
			if (operation === 'cut') clipboardStore.clear();
			data.refetchDirectory();
		} catch (error) {
			console.error('Paste operation failed:', error);
			toastStore.error(errorMessage(error, 'Paste failed'));
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
			renameDialog = { open: false, file: null, newName: '' };
			data.refetchDirectory();
		} catch (error) {
			console.error('Rename failed:', error);
			toastStore.error(errorMessage(error, 'Rename failed'));
		}
	}

	/** Files go at once; folders become background jobs. */
	async function deleteItems(items: FileInfo[]) {
		if (items.length === 0) return;

		try {
			for (const item of items) {
				if (item.isDir) {
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
		await deleteItems(deleteDialog.items);
		deleteDialog = { open: false, items: [] };
	}

	return {
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
		get shareDialog() {
			return shareDialog;
		},
		get folderShareDialog() {
			return folderShareDialog;
		},
		get favoritePaths() {
			return favoritePaths;
		},
		get cutPaths() {
			return cutPaths;
		},
		get canPaste() {
			return clipboardStore.hasItems;
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
			shareDialog = { open: false, file: null };
		},
		closeFolderShare() {
			folderShareDialog = { open: false, folder: null };
		},
		confirmCreate,
		confirmRename,
		confirmDelete,

		openFile(file: FileInfo) {
			if (file.isDir) {
				location.navigate(file.path);
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
				case 'new-file':
					openCreate('file');
					break;
				case 'new-folder':
					openCreate('directory');
					break;
				case 'copy':
					clipboardStore.copy(items);
					break;
				case 'cut':
					clipboardStore.cut(items);
					break;
				case 'paste':
					await paste();
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
						deleteDialog = { open: true, items };
					} else {
						await deleteItems(items);
					}
					break;
				case 'download':
					download(items);
					break;
				case 'share':
					if (only?.isDir) folderShareDialog = { open: true, folder: only };
					else if (only) shareDialog = { open: true, file: only };
					break;
				case 'properties':
					if (only) propertiesDialog = { open: true, file: only };
					break;
			}
		}
	};
}

export type BrowseActions = ReturnType<typeof useBrowseActions>;
