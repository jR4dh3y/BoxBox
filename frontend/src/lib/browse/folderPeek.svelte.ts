/**
 * What a folder holds, drawn inside its grid tile as Windows and the desktop app show it: a few
 * of its items, pictures first. Asked for by tiles as they are drawn, and kept by the folder's
 * modified time, which changes when an entry is added or removed.
 */
import { SvelteMap } from 'svelte/reactivity';
import { peekDirectory, type FileInfo } from '#lib/api/files.js';

/** More folders than a screen of tiles shows; past it the cache starts again. */
const CAPACITY = 1024;
/** A failed read is tried again when its tile is next drawn after this long. */
const RETRY_MS = 10_000;

export type PeekLoader = (path: string, includeHidden: boolean) => Promise<FileInfo[]>;

export class FolderPeeks {
	private readonly peeks = new SvelteMap<string, readonly FileInfo[]>();
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- Requests in flight are not drawn.
	private readonly pending = new Set<string>();
	// When each failed read failed; a failure may be brief, so it is not kept for good.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- Not drawn.
	private readonly failed = new Map<string, number>();
	private readonly load: PeekLoader;
	private readonly now: () => number;

	constructor(load: PeekLoader, now: () => number = Date.now) {
		this.load = load;
		this.now = now;
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
		if (!folder.isDir || this.pending.has(key)) return;
		const failedAt = this.failed.get(key);
		const known = this.peeks.has(key);
		if (known && (failedAt === undefined || this.now() - failedAt < RETRY_MS)) return;
		this.pending.add(key);
		void this.load(folder.path, includeHidden).then(
			(items) => this.settle(key, items, false),
			// Unreadable, or a server without peeks: drawn as a plain folder for now.
			() => this.settle(key, [], true)
		);
	}

	private settle(key: string, items: readonly FileInfo[], failed: boolean): void {
		if (!this.pending.delete(key)) return;
		if (this.peeks.size >= CAPACITY) this.clear();
		this.peeks.set(key, items);
		if (failed) this.failed.set(key, this.now());
		else this.failed.delete(key);
	}

	clear(): void {
		this.peeks.clear();
		this.pending.clear();
		this.failed.clear();
	}
}

/** One cache for every grid, so a folder's peek survives switching views. */
export const folderPeeks = new FolderPeeks((path, hidden) => peekDirectory(path, hidden));
