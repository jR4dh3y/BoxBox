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
import { getDirectoryArchiveUrl, getDownloadUrl, type FileInfo } from '#lib/api/files.js';
import { clipboardStore } from '#lib/stores/clipboard.svelte.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { toastStore } from '#lib/stores/toast.svelte.js';
import { canPreview, getFileTypeDescription } from '#lib/utils/fileTypes.js';
import type { BrowseData } from './data.svelte';
import type { BrowseLocation } from './location.svelte';
import type { BrowseSelection } from './selection.svelte';
import { createNameProblem, nameProblem, unusedName } from './names.js';
import { browseRequests } from './requests.svelte';

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

function parentOf(path: string): string {
	return path.substring(0, path.lastIndexOf('/'));
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

	/**
	 * The names already in `folder`, other than `except`'s. Empty when its listing is not held,
	 * as for a search result elsewhere; the server then refuses a name that is taken.
	 */
	function siblingNames(folder: string, except?: FileInfo): string[] {
		const names = data.namesIn(folder) ?? [];
		return except ? names.filter((name) => name !== except.name) : names;
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
		const taken = siblingNames(location.path);
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
		const problem = createNameProblem(name, siblingNames(location.path));
		if (problem) {
			createDialog.problem = problem;
			return;
		}

		const { type } = createDialog;
		const folder = location.path;
		const signal = browseRequests.sessionSignal;
		try {
			const { createItem } = await import('./mutations.js');
			const created = await createItem(type, folder, name, signal);
			if (signal.aborted) return;
			closeCreate();
			// The new item shows selected in the folder, so success needs no notice.
			selection.set(new SvelteSet([created.path]));
			data.refresh();
		} catch (error) {
			if (!signal.aborted) toastStore.error(errorMessage(error, 'Create failed'));
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
		const oldPath = current.file.path;
		const parentPath = parentOf(oldPath);
		if (
			newName === current.file.name ||
			nameProblem(newName, siblingNames(parentPath, current.file))
		)
			return;

		const newPath = parentPath ? `${parentPath}/${newName}` : newName;

		const signal = browseRequests.sessionSignal;
		try {
			const { renameItem } = await import('./mutations.js');
			await renameItem(oldPath, newPath, signal);
			if (signal.aborted) return;
			data.refetchDirectory();
		} catch (error) {
			if (signal.aborted) return;
			console.error('Rename failed:', error);
			toastStore.error(errorMessage(error, 'Rename failed'));
		}
	}

	/** Files go at once; folders become background jobs. */
	async function deleteItems(items: FileInfo[], signal = browseRequests.sessionSignal) {
		if (items.length === 0) return;

		try {
			const { deleteItems: deleteSelection } = await import('./mutations.js');
			await deleteSelection(items, signal);
			if (signal.aborted) return;
			selection.clear();
			data.refetchDirectory();
		} catch (error) {
			if (signal.aborted) return;
			console.error('Delete failed:', error);
			toastStore.error(errorMessage(error, 'Delete failed'));
		}
	}

	async function confirmDelete() {
		const signal = browseRequests.sessionSignal;
		await deleteItems(deleteDialog.items, signal);
		if (!signal.aborted) deleteDialog = { open: false, items: [] };
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
			createDialog.problem = createNameProblem(name.trim(), siblingNames(location.path));
		},
		setRenameName(name: string) {
			if (renaming.kind !== 'renaming') return;
			renaming = {
				...renaming,
				name,
				problem: nameProblem(name.trim(), siblingNames(parentOf(renaming.file.path), renaming.file))
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
						// The sidebar shows the pin; only failures notify, as in the desktop app.
						settingsStore.pinFavoriteFolder({ name: only.name, path: only.path });
					}
					break;
				case 'unpin':
					if (only?.isDir) {
						settingsStore.unpinFavoriteFolder(only.path);
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
