/**
 * What a folder holds, drawn inside its grid tile as Windows and the desktop app show it: a few
 * of its items, pictures first. Asked for by tiles as they are drawn, and kept by the folder's
 * modified time, which changes when an entry is added or removed.
 */
import { SvelteMap } from 'svelte/reactivity';
import { peekDirectory, type FileInfo } from '#lib/api/files.js';

/** More folders than a screen of tiles shows; past it the cache starts again. */
const CAPACITY = 1024;

export type PeekLoader = (path: string, includeHidden: boolean) => Promise<FileInfo[]>;

export class FolderPeeks {
	private readonly peeks = new SvelteMap<string, readonly FileInfo[]>();
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- Requests in flight are not drawn.
	private readonly pending = new Set<string>();
	private readonly load: PeekLoader;

	constructor(load: PeekLoader) {
		this.load = load;
	}

	private key(folder: FileInfo, includeHidden: boolean): string {
		return JSON.stringify([folder.path, folder.modTime, includeHidden]);
	}

	/** The items drawn inside `folder`, once read; empty for an empty or unreadable folder. */
	get(folder: FileInfo, includeHidden: boolean): readonly FileInfo[] | undefined {
		return this.peeks.get(this.key(folder, includeHidden));
	}

	/** Reads what a folder holds, unless it is known or being read. */
	request(folder: FileInfo, includeHidden: boolean): void {
		const key = this.key(folder, includeHidden);
		if (!folder.isDir || this.peeks.has(key) || this.pending.has(key)) return;
		this.pending.add(key);
		void this.load(folder.path, includeHidden).then(
			(items) => this.settle(key, items),
			// Unreadable, or a server without peeks: drawn as a plain folder.
			() => this.settle(key, [])
		);
	}

	private settle(key: string, items: readonly FileInfo[]): void {
		if (!this.pending.delete(key)) return;
		if (this.peeks.size >= CAPACITY) this.peeks.clear();
		this.peeks.set(key, items);
	}

	clear(): void {
		this.peeks.clear();
		this.pending.clear();
	}
}

/** One cache for every grid, so a folder's peek survives switching views. */
export const folderPeeks = new FolderPeeks((path, hidden) => peekDirectory(path, hidden));
