import type { FileDropHandler, FileDropTarget } from './fileDrag';

/** Highlights the destination under the pointer; the handler decides and performs the drop. */
export function createDropZone(handler: () => FileDropHandler | null | undefined): FileDropTarget {
	let overPath = $state<string | null>(null);

	return {
		get overPath() {
			return overPath;
		},
		over(event, destination) {
			const accepted = handler()?.over(event, destination) ?? false;
			if (accepted) overPath = destination;
			else if (overPath === destination) overPath = null;
			return accepted;
		},
		leave(event) {
			const area = event.currentTarget;
			const next = event.relatedTarget;
			if (area instanceof Node && next instanceof Node && area.contains(next)) return;
			overPath = null;
		},
		refuse(event) {
			event.stopPropagation();
			overPath = null;
		},
		drop(event, destination) {
			overPath = null;
			handler()?.drop(event, destination);
		}
	};
}
