/**
 * The card a row or tile shows when the pointer rests on it, as the desktop app and File Pilot
 * show it: what the item is, how big it is, and what a folder holds, without selecting it.
 */
import type { FileInfo } from '#lib/api/files.js';

/** The desktop app's: GPUI's 500 ms tooltip delay and 250 ms more, so a pass across the grid shows nothing. */
export const HOVER_CARD_DELAY_MS = 750;

export interface FolderTotals {
	files: number;
	folders: number;
	bytes: number;
}

export type HoverContents =
	{ kind: 'loading' } | { kind: 'ready'; totals: FolderTotals } | { kind: 'failed' };

export type HoverCardState =
	| { kind: 'hidden' }
	| { kind: 'waiting'; item: FileInfo }
	| {
			kind: 'shown';
			item: FileInfo;
			x: number;
			y: number;
			/** Set for search results, which come from many folders. */
			location: string | null;
			/** Null for a file. */
			contents: HoverContents | null;
	  };

/** A server folder's own listing, one level down, as the desktop app counts a server's folder. */
export function folderTotals(items: readonly FileInfo[]): FolderTotals {
	let files = 0;
	let folders = 0;
	let bytes = 0;
	for (const item of items) {
		if (item.isDir) folders++;
		else {
			files++;
			bytes += item.size;
		}
	}
	return { files, folders, bytes };
}

export function describeTotals({ files, folders }: FolderTotals): string {
	const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
	const parts = folders > 0 ? [count(folders, 'folder', 'folders')] : [];
	parts.push(count(files, 'file', 'files'));
	return parts.join(', ');
}

/** What a row or tile tells the card. */
export interface HoverTarget {
	rest(item: FileInfo, at: HoverPoint): void;
	hide(): void;
}

export interface HoverPoint {
	x: number;
	y: number;
	touch: boolean;
}

export function hoverPoint(event: PointerEvent): HoverPoint {
	return { x: event.clientX, y: event.clientY, touch: event.pointerType === 'touch' };
}

export interface HoverCardOptions {
	/** Whether Settings show the card. */
	enabled: () => boolean;
	/** Where a search result lives, or null outside search. */
	location: (item: FileInfo) => string | null;
	/** A folder's listing; it may come from the cache the hover prefetch filled. */
	listFolder: (path: string) => Promise<readonly FileInfo[]>;
	delayMs?: number;
}

export class HoverCard implements HoverTarget {
	state = $state.raw<HoverCardState>({ kind: 'hidden' });
	private timer: ReturnType<typeof setTimeout> | undefined;
	private pointer = { x: 0, y: 0 };
	private generation = 0;
	private readonly options: HoverCardOptions;

	constructor(options: HoverCardOptions) {
		this.options = options;
	}

	/** The pointer reached an item, or moved over it; the card waits for it to rest. */
	rest(item: FileInfo, at: HoverPoint): void {
		if (!this.options.enabled() || at.touch) return;
		this.pointer = { x: at.x, y: at.y };
		const current = this.state;
		if (current.kind !== 'hidden' && current.item.path === item.path) {
			// Moving inside the item restarts the wait until the card is up.
			if (current.kind === 'waiting') this.wait(item);
			return;
		}
		this.wait(item);
	}

	/** The pointer left the item, pressed, or a menu, drag or dialog covered it. */
	hide(): void {
		clearTimeout(this.timer);
		this.generation++;
		if (this.state.kind !== 'hidden') this.state = { kind: 'hidden' };
	}

	private wait(item: FileInfo): void {
		clearTimeout(this.timer);
		const generation = ++this.generation;
		this.state = { kind: 'waiting', item };
		this.timer = setTimeout(
			() => this.show(item, generation),
			this.options.delayMs ?? HOVER_CARD_DELAY_MS
		);
	}

	private show(item: FileInfo, generation: number): void {
		if (generation !== this.generation) return;
		this.state = {
			kind: 'shown',
			item,
			x: this.pointer.x,
			y: this.pointer.y,
			location: this.options.location(item),
			contents: item.isDir ? { kind: 'loading' } : null
		};
		if (!item.isDir) return;
		void this.options.listFolder(item.path).then(
			(items) => this.settle(generation, { kind: 'ready', totals: folderTotals(items) }),
			() => this.settle(generation, { kind: 'failed' })
		);
	}

	private settle(generation: number, contents: HoverContents): void {
		const current = this.state;
		if (generation === this.generation && current.kind === 'shown')
			this.state = { ...current, contents };
	}
}
