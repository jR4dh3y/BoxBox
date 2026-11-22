import { useBrowsePaste } from './paste.svelte';
import {
	isNativePath,
	isLocalDesktop,
	openDesktopFile,
	supportsServerTransfer
} from '#lib/api/desktop.js';
import {
	beginBrowseClipboardPublication,
	finishBrowseClipboardPublication,
	publishBrowseClipboardInOrder
} from './clipboardPublication';
import { SvelteSet } from 'svelte/reactivity';
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
import { clipboardStore } from '#lib/stores/clipboard.svelte.js';
import { jobsStore } from '#lib/stores/jobs.svelte.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { toastStore } from '#lib/stores/toast.svelte.js';
import { canPreview, getFileTypeDescription } from '#lib/utils/fileTypes.js';
import type { BrowseData } from './data.svelte';
import type { BrowseLocation } from './location.svelte';
import type { BrowseSelection } from './selection.svelte';
import { nameProblem, unusedName } from './names.js';

function errorMessage(error: unknown, fallback: string): string {
	return error instanceof Error ? error.message : fallback;
}

function download(items: FileInfo[]) {
	for (const item of items) {
		window.open(
			item.isDir ? getDirectoryArchiveUrl(item.path) : getDownloadUrl(item.path),
			'_blank'
		);
	}
}

/** An item being renamed where its name is drawn, with why the typed name cannot be used. */
export type Renaming =
	{ kind: 'idle' } | { kind: 'renaming'; file: FileInfo; name: string; problem: string | null };

export interface CreateDialog {
	open: boolean;
	type: 'file' | 'directory';
	name: string;
	/** Why the typed name cannot be used, or null. */
	problem: string | null;
}

/** What the user does to files and folders: dialogs, file operations, clipboard and preview. */
export function useBrowseActions(
	location: BrowseLocation,
	data: BrowseData,
	selection: BrowseSelection
) {
	const pasteController = useBrowsePaste(location, data);
	let openWithFile = $state<FileInfo | null>(null);
	let previewFile = $state<FileInfo | null>(null);
	let createDialog = $state<CreateDialog>({ open: false, type: 'file', name: '', problem: null });
	let renaming = $state.raw<Renaming>({ kind: 'idle' });

	/** The names already in the open folder, other than `except`. */
	function siblingNames(except?: string): string[] {
		return data.displayItems.filter((item) => item.path !== except).map((item) => item.name);
	}
	let deleteDialog = $state<{ open: boolean; items: FileInfo[] }>({ open: false, items: [] });
	// The items Properties describes; null when it is closed.
	let propertiesItems = $state.raw<FileInfo[] | null>(null);
	// The folder whose look is being chosen; null when Customize Folder is closed.
	let customizeFolder = $state<FileInfo | null>(null);
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
		createDialog = { open: false, type: 'file', name: '', problem: null };
	}

	function openCreate(type: 'file' | 'directory') {
		if (!data.canCreate) {
			toastStore.error('Cannot create items in this location');
			return;
		}
		// A name not taken yet, as the desktop app suggests one.
		const taken = siblingNames();
		const name = unusedName(
			type === 'file' ? 'untitled.txt' : 'New Folder',
			type === 'directory',
			(candidate) => taken.includes(candidate)
		);
		createDialog = { open: true, type, name, problem: null };
	}

	async function confirmCreate() {
		if (!location.path) return;
		const name = createDialog.name.trim();
		const problem = nameProblem(name, siblingNames());
		if (problem) {
			createDialog.problem = problem;
			return;
		}

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

	async function publishClipboard(items: FileInfo[], operation: 'copy' | 'cut') {
		const revision = beginBrowseClipboardPublication();
		if (operation === 'copy') clipboardStore.copy(items);
		else clipboardStore.cut(items);
		if (!isLocalDesktop() || items.some((item) => !isNativePath(item.path))) {
			finishBrowseClipboardPublication(revision, 'failed');
			return;
		}
		try {
			const { setDesktopClipboard } = await import('#lib/api/desktopClipboard.js');
			await publishBrowseClipboardInOrder(() =>
				setDesktopClipboard(
					items.map((item) => item.path),
					operation
				)
			);
			finishBrowseClipboardPublication(revision, 'published');
		} catch {
			finishBrowseClipboardPublication(revision, 'failed');
		}
	}

	async function confirmRename() {
		const current = renaming;
		if (current.kind !== 'renaming') return;
		renaming = { kind: 'idle' };
		const newName = current.name.trim();
		// An unchanged name is not a rename.
		if (newName === current.file.name || nameProblem(newName, siblingNames(current.file.path)))
			return;

		const oldPath = current.file.path;
		const parentPath = oldPath.substring(0, oldPath.lastIndexOf('/'));
		const newPath = parentPath ? `${parentPath}/${newName}` : newName;

		try {
			await rename(oldPath, newPath);
			if (isNativePath(oldPath)) {
				const { desktopUndo } = await import('./undo.svelte');
				desktopUndo.rememberRename(oldPath, newPath);
			}
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
			const trashed: string[] = [];
			for (const item of items) {
				if (isNativePath(item.path)) {
					const { trashDesktopFile } = await import('#lib/api/desktopTrash.js');
					trashed.push((await trashDesktopFile(item.path)).id);
					const { desktopUndo } = await import('./undo.svelte');
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
		await deleteItems(deleteDialog.items);
		deleteDialog = { open: false, items: [] };
	}

	return {
		paste: pasteController,
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
		get renaming() {
			return renaming;
		},
		get deleteDialog() {
			return deleteDialog;
		},
		get propertiesItems() {
			return propertiesItems;
		},
		get shareItem() {
			return shareItem;
		},
		get customizeFolder() {
			return customizeFolder;
		},
		closeCustomize() {
			customizeFolder = null;
		},
		get favoritePaths() {
			return favoritePaths;
		},
		get cutPaths() {
			return cutPaths;
		},
		get canPaste() {
			return clipboardStore.hasItems || (isNativePath(location.path) && data.canCreate);
		},

		setCreateName(name: string) {
			createDialog.name = name;
			createDialog.problem = nameProblem(name.trim(), siblingNames());
		},
		setRenameName(name: string) {
			if (renaming.kind !== 'renaming') return;
			renaming = {
				...renaming,
				name,
				problem: nameProblem(name.trim(), siblingNames(renaming.file.path))
			};
		},
		closeCreate,
		cancelRename() {
			renaming = { kind: 'idle' };
		},
		closeDelete() {
			deleteDialog = { open: false, items: [] };
		},
		closeProperties() {
			propertiesItems = null;
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
				void openDesktopFile(file.path).catch((error) =>
					toastStore.error(errorMessage(error, 'Open failed'))
				);
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
			if (
				data.isReadOnly &&
				['new-file', 'new-folder', 'cut', 'paste', 'rename', 'delete'].includes(action)
			) {
				toastStore.error('This location is read-only or unavailable');
				return;
			}
			const [only] = items.length === 1 ? items : [];
			switch (action) {
				case 'open-with':
					if (only && isNativePath(only.path)) openWithFile = only;
					break;
				case 'preview':
					if (only) previewFile = only;
					break;
				case 'open-terminal':
					if (only?.isDir && isNativePath(only.path)) {
						const { openDesktopTerminal } = await import('#lib/api/desktopApps.js');
						try {
							await openDesktopTerminal(only.path);
						} catch (error) {
							toastStore.error(errorMessage(error, 'Terminal could not open'));
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
					await publishClipboard(items, 'copy');
					break;
				case 'cut':
					await publishClipboard(items, 'cut');
					break;
				case 'paste':
					await pasteController.paste();
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
					if (only) renaming = { kind: 'renaming', file: only, name: only.name, problem: null };
					break;
				case 'delete':
					if (settingsStore.current.confirmDelete) {
						deleteDialog = { open: true, items };
					} else {
						await deleteItems(items);
					}
					break;
				case 'download':
					if (supportsServerTransfer(items)) download(items);
					break;
				case 'share':
					if (only && supportsServerTransfer(items)) shareItem = only;
					break;
				case 'properties':
					if (items.length > 0) propertiesItems = items;
					break;
				case 'customize':
					if (only?.isDir) customizeFolder = only;
					break;
			}
		}
	};
}

export type BrowseActions = ReturnType<typeof useBrowseActions>;
