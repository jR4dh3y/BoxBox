/**
 * What the browser's history holds for the browse page, which the page cannot read back: each
 * entry's folder, and the selection left there, so Back and Forward return to the selection
 * and their menus can name the places. Kept per browser tab, so a reload keeps it.
 */
import { isRecord } from '#lib/api/parse.js';

export interface TrailEntry {
	path: string;
	/** What was selected when the page left the entry. */
	selected: readonly string[];
}

export interface TrailStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

const KEY = 'boxbox_history_trail';
/** Older entries are forgotten; Back still works, its menu shows fewer places. */
const LIMIT = 200;

function parseEntry(value: unknown): TrailEntry | null {
	if (!isRecord(value) || typeof value.path !== 'string' || !Array.isArray(value.selected))
		return null;
	return {
		path: value.path,
		selected: value.selected.filter((path): path is string => typeof path === 'string')
	};
}

export class HistoryTrail {
	private entries: (TrailEntry | null)[] = [];
	private readonly storage: TrailStorage | null;

	constructor(storage: TrailStorage | null) {
		this.storage = storage;
		try {
			const saved: unknown = JSON.parse(storage?.getItem(KEY) ?? '[]');
			if (Array.isArray(saved)) this.entries = saved.map(parseEntry);
		} catch {
			this.entries = [];
		}
	}

	private save(): void {
		try {
			this.storage?.setItem(KEY, JSON.stringify(this.entries));
		} catch {
			// A full or blocked storage only loses the menus and the restored selection.
		}
	}

	/** The page arrived at `index`; a new entry replaces what was forward of it. */
	arrive(index: number, path: string, fresh: boolean): void {
		if (fresh) this.entries = this.entries.slice(0, index);
		const known = this.entries[index];
		this.entries[index] = known && known.path === path ? known : { path, selected: [] };
		// Positions are history indexes, so old entries are dropped in place.
		for (let old = 0; old < this.entries.length - LIMIT; old++) this.entries[old] = null;
		this.save();
	}

	/** The page is leaving `index` with these items selected. */
	leave(index: number, selected: Iterable<string>): void {
		const entry = this.entries[index];
		if (!entry) return;
		this.entries[index] = { ...entry, selected: [...selected] };
		this.save();
	}

	at(index: number): TrailEntry | null {
		return this.entries[index] ?? null;
	}

	/** Known entries behind (`back`) or ahead of `index`, nearest first, with their indexes. */
	around(index: number, back: boolean, last: number): { index: number; entry: TrailEntry }[] {
		const found: { index: number; entry: TrailEntry }[] = [];
		if (back) {
			for (let at = index - 1; at >= 0; at--) {
				const entry = this.entries[at];
				if (entry) found.push({ index: at, entry });
			}
		} else {
			for (let at = index + 1; at <= last; at++) {
				const entry = this.entries[at];
				if (entry) found.push({ index: at, entry });
			}
		}
		return found;
	}
}
