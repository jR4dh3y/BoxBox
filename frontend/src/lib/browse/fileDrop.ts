import { isLocalDesktop } from '#lib/api/desktop.js';
import { toastStore } from '#lib/stores/toast.svelte.js';
import type { BrowseActions } from './actions.svelte';
import type { BrowseData } from './data.svelte';
import {
	canDropInto,
	draggedSources,
	dropOperation,
	fileDragKind,
	readDroppedLocalFiles,
	readFileDrag,
	type FileDropHandler
} from './fileDrag';
import type { useBrowseUploads } from './uploads.svelte';

/**
 * Drops onto a pane, its folders, or the sidebar. BoxBox items and, in the desktop app,
 * local files from other apps become copy or move jobs; the web app uploads other files.
 */
export function useFileDrop(
	data: BrowseData,
	actions: BrowseActions,
	uploads: ReturnType<typeof useBrowseUploads>
): FileDropHandler {
	function accepts(event: DragEvent, destination: string): boolean {
		const kind = fileDragKind(event.dataTransfer);
		if (!kind || !data.isWritable(destination)) return false;
		const sources = kind === 'files' ? draggedSources() : null;
		return canDropInto(sources, destination, dropOperation(kind, event, destination));
	}

	return {
		over(event, destination) {
			if (!accepts(event, destination)) return false;
			event.preventDefault();
			const kind = fileDragKind(event.dataTransfer);
			if (event.dataTransfer && kind)
				event.dataTransfer.dropEffect = dropOperation(kind, event, destination);
			return true;
		},
		drop(event, destination) {
			const dataTransfer = event.dataTransfer;
			const kind = fileDragKind(dataTransfer);
			if (!dataTransfer || !kind || !accepts(event, destination)) return;
			event.preventDefault();
			event.stopPropagation();
			const operation = dropOperation(kind, event, destination);
			if (kind === 'files') {
				const sources = readFileDrag(dataTransfer);
				if (canDropInto(sources, destination, operation)) {
					void actions.paste.transfer(operation, sources, destination);
				}
				return;
			}
			if (isLocalDesktop()) {
				const sources = readDroppedLocalFiles(dataTransfer.getData('text/uri-list'));
				if (sources.length > 0) {
					void actions.paste.transfer('copy', sources, destination);
					return;
				}
				if (dataTransfer.files.length === 0) {
					toastStore.error('Only files can be dropped here');
					return;
				}
			}
			uploads.uploadDropped(dataTransfer, destination);
		}
	};
}
