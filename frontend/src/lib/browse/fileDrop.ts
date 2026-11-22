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
import type { TransferSource } from './transfers';
import type { BrowseActions } from './actions.svelte';
import type { BrowseData } from './data.svelte';
import { parseRemotePath } from '#lib/api/remotePaths.js';

/** A drop with Alt held asks what to do with the items, at the drop point. */
export interface DropQuestion {
	x: number;
	y: number;
	items: TransferSource[];
	destination: string;
	/** Moving is offered only within one server. */
	canMove: boolean;
}

export function useFileDrop(
	data: BrowseData,
	actions: BrowseActions,
	ask: (question: DropQuestion) => void = () => {}
): FileDropHandler {
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
			const sources = draggedSources();
			let operation = dropOperation(kind, event, destination);
			// Alt asks on drop, and into the items' own folder it can still offer Copy Here.
			if (event.altKey && !canDropInto(sources, destination, operation)) operation = 'copy';
			if (!canDropInto(sources, destination, operation)) return false;
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
			if (items.length && event.altKey) {
				if (!canDropInto(items, destination, 'copy')) return;
				const canMove =
					dropOperation(kind, { ctrlKey: false }, destination, items) === 'move' &&
					canDropInto(items, destination, 'move');
				ask({ x: event.clientX, y: event.clientY, items, destination, canMove });
				return;
			}
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
