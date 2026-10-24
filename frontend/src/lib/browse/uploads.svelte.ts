import { toastStore } from '#lib/stores/toast.svelte.js';
import { uploadStore } from '#lib/stores/upload.svelte.js';
import { entriesFromDataTransfer, entriesFromFiles } from '#lib/utils/uploadEntries.js';
import type { BrowseData } from './data.svelte';
import type { BrowseLocation } from './location.svelte';

/** Picking files or folders to upload, and uploading files dropped from other apps. */
export function useBrowseUploads(location: BrowseLocation, data: BrowseData) {
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
	// Uploads keep their destination while the user navigates, so refresh every folder.
	uploadStore.onRefreshNeeded = () => {
		if (refreshTimer) clearTimeout(refreshTimer);
		refreshTimer = setTimeout(() => data.refreshAll(), 250);
	};

	function uploadDestination(): string | null {
		if (!data.canCreate) {
			toastStore.error(
				data.isReadOnly
					? 'Cannot upload to read-only location'
					: 'Navigate to a folder first to upload files'
			);
			return null;
		}
		return location.path;
	}

	return {
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
			const destination = uploadDestination();
			if (destination !== null && input.files && input.files.length > 0) {
				uploadStore.addFiles(entriesFromFiles(input.files), destination);
			}
			// Reset so the same file can be picked again
			input.value = '';
		},
		/** Uploads files dropped from another app; reading a dropped folder is async. */
		uploadDropped(dataTransfer: DataTransfer, destination: string) {
			entriesFromDataTransfer(dataTransfer)
				.then((entries) => {
					if (entries.length > 0) uploadStore.addFiles(entries, destination);
				})
				.catch(() => toastStore.error('Unable to read the dropped items'));
		}
	};
}
