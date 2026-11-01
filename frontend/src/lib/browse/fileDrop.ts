import { isLocalDesktop } from '#lib/api/desktop.js';
import { entriesFromDataTransfer } from '#lib/utils/uploadEntries.js';
import { uploadStore } from '#lib/stores/upload.svelte.js';
import { toastStore } from '#lib/stores/toast.svelte.js';
import {
	canDropInto,
	draggedSources,
	dropOperation,
	fileDragKind,
	readFileDrag,
	readDroppedLocalFiles,
	type FileDropHandler
} from './fileDrag';
import type { BrowseActions } from './actions.svelte';
import type { BrowseData } from './data.svelte';
import { parseRemotePath } from '#lib/api/remotePaths.js';

export function useFileDrop(data: BrowseData, actions: BrowseActions): FileDropHandler {
	function allowed(destination: string) {
		const source = parseRemotePath(destination);
		if (!destination || (source.kind === 'remote' && !source.path)) return false;
		const mount = [...data.roots]
			.sort((a, b) => b.name.length - a.name.length)
			.find((root) => destination === root.name || destination.startsWith(root.name + '/'));
		return !!mount && !mount.readOnly;
	}
	return {
		over(event, destination) {
			const kind = fileDragKind(event.dataTransfer);
			if (!kind || !allowed(destination)) return false;
			const operation = dropOperation(kind, event, destination);
			if (!canDropInto(draggedSources(), destination, operation)) return false;
			event.preventDefault();
			if (event.dataTransfer) event.dataTransfer.dropEffect = operation;
			return true;
		},
		drop(event, destination) {
			const transfer = event.dataTransfer;
			const kind = fileDragKind(transfer);
			event.preventDefault();
			event.stopPropagation();
			if (!transfer || !kind || !allowed(destination)) return;
			const items =
				kind === 'files'
					? readFileDrag(transfer)
					: isLocalDesktop()
						? readDroppedLocalFiles(transfer.getData('text/uri-list'))
						: [];
			const operation = dropOperation(kind, event, destination, items);
			if (!canDropInto(items, destination, operation)) return;
			if (items.length) {
				void actions.paste.transfer(operation, items, destination);
				return;
			}
			if (kind === 'external')
				void entriesFromDataTransfer(transfer)
					.then((entries) => uploadStore.addFiles(entries, destination))
					.catch(() => toastStore.error('Unable to read the dropped items'));
		}
	};
}
