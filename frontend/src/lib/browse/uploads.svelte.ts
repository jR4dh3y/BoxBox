import { toastStore } from '$lib/stores/toast.svelte';
import { uploadStore } from '$lib/stores/upload.svelte';
import {
	entriesFromDataTransfer,
	entriesFromFiles,
	type UploadEntry
} from '$lib/utils/uploadEntries';
import type { BrowseData } from './data.svelte';
import type { BrowseLocation } from './location.svelte';

/** Picking and dropping files or folders to upload into the current folder. */
export function useBrowseUploads(location: BrowseLocation, data: BrowseData) {
	let isDragOver = $state(false);
	let fileInput = $state<HTMLInputElement>();
	let folderInput = $state<HTMLInputElement>();
	let refreshTimer: ReturnType<typeof setTimeout> | undefined;

	uploadStore.onComplete = (fileName, success, error) => {
		if (success) {
			toastStore.success(`${fileName} uploaded successfully`);
		} else {
			toastStore.error(`Upload failed: ${error || 'Unknown error'}`);
		}
	};
	uploadStore.onRefreshNeeded = () => {
		if (refreshTimer) clearTimeout(refreshTimer);
		refreshTimer = setTimeout(() => data.refresh(), 250);
	};

	function uploadDestination(): string | null {
		if (data.isAtRoot) {
			toastStore.warning('Navigate to a folder first to upload files');
			return null;
		}
		if (data.isReadOnly) {
			toastStore.error('Cannot upload to read-only location');
			return null;
		}
		return location.path;
	}

	function start(entries: UploadEntry[], destination = uploadDestination()) {
		if (destination === null) return;
		uploadStore.addFiles(entries, destination);
	}

	return {
		get isDragOver() {
			return isDragOver;
		},
		get fileInput() {
			return fileInput;
		},
		set fileInput(element: HTMLInputElement | undefined) {
			fileInput = element;
		},
		get folderInput() {
			return folderInput;
		},
		set folderInput(element: HTMLInputElement | undefined) {
			folderInput = element;
		},
		openPicker: () => fileInput?.click(),
		openFolderPicker: () => folderInput?.click(),
		handleInputChange(event: Event & { currentTarget: HTMLInputElement }) {
			const input = event.currentTarget;
			if (input.files && input.files.length > 0) start(entriesFromFiles(input.files));
			// Reset so the same file can be picked again
			input.value = '';
		},
		handleDragOver(event: DragEvent) {
			event.preventDefault();
			event.stopPropagation();
			if (!data.isAtRoot) isDragOver = true;
		},
		handleDragLeave(event: DragEvent) {
			event.preventDefault();
			event.stopPropagation();
			isDragOver = false;
		},
		handleDrop(event: DragEvent) {
			event.preventDefault();
			event.stopPropagation();
			isDragOver = false;
			const dataTransfer = event.dataTransfer;
			if (!dataTransfer) return;
			// Reading a dropped folder is async; upload to where it was dropped.
			const destination = uploadDestination();
			if (destination === null) return;
			entriesFromDataTransfer(dataTransfer)
				.then((entries) => {
					if (entries.length > 0) start(entries, destination);
				})
				.catch(() => toastStore.error('Unable to read the dropped items'));
		}
	};
}
