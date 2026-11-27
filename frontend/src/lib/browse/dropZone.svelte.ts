import type { FileDropHandler, FileDropTarget } from './fileDrag';
import { SpringLoad } from './springLoad';

export interface DropZone extends FileDropTarget {
	/** Something that is not a destination, such as Back, which a held drag uses. */
	hold(key: string, act: () => void): void;
	/** The drag left a hold, ended or dropped. */
	release(): void;
}

/**
 * Highlights the destination under the pointer; the handler decides and performs the drop.
 * A drag that rests on a destination opens it through `open`, whether or not it takes the drop.
 */
export function createDropZone(
	handler: () => FileDropHandler | null | undefined,
	open: (destination: string) => void = () => {}
): DropZone {
	let overPath = $state<string | null>(null);
	const spring = new SpringLoad();

	return {
		get overPath() {
			return overPath;
		},
		over(event, destination) {
			spring.rest(destination, () => open(destination));
			const accepted = handler()?.over(event, destination) ?? false;
			if (accepted) overPath = destination;
			else if (overPath === destination) overPath = null;
			return accepted;
		},
		leave(event) {
			const area = event.currentTarget;
			const next = event.relatedTarget;
			if (area instanceof Node && next instanceof Node && area.contains(next)) return;
			spring.cancel();
			overPath = null;
		},
		refuse(event) {
			event.stopPropagation();
			spring.cancel();
			overPath = null;
		},
		drop(event, destination) {
			spring.cancel();
			overPath = null;
			handler()?.drop(event, destination);
		},
		hold(key, act) {
			spring.rest(key, act);
		},
		release() {
			spring.cancel();
		}
	};
}
