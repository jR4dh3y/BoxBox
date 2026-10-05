import { toastStore } from '$lib/stores/toast.svelte';
import { uploadStore } from '$lib/stores/upload.svelte';
import type { BrowseData } from './data.svelte';
import type { BrowseLocation } from './location.svelte';

/** Picking and dropping files to upload into the current folder. */
export function useBrowseUploads(location: BrowseLocation, data: BrowseData) {
	let isDragOver = $state(false);
	let fileInput = $state<HTMLInputElement>();
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

	function start(files: File[]) {
		if (data.isAtRoot) {
			toastStore.warning('Navigate to a folder first to upload files');
			return;
		}
		if (data.isReadOnly) {
			toastStore.error('Cannot upload to read-only location');
			return;
		}
		uploadStore.addFiles(files, location.path);
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
		openPicker: () => fileInput?.click(),
		handleInputChange(event: Event & { currentTarget: HTMLInputElement }) {
			const input = event.currentTarget;
			if (input.files && input.files.length > 0) start(Array.from(input.files));
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
			const files = event.dataTransfer?.files;
			if (files && files.length > 0) start(Array.from(files));
		}
	};
}
