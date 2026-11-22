/**
 * Moving through a folder from the keyboard, as GTK file managers and the desktop app do:
 * arrows, Home, End and the page keys; Shift extends the selection and Ctrl moves the cursor
 * alone; Ctrl+Space toggles; typing a name jumps to it. Works on positions, so it reaches
 * items the view has not drawn.
 */

/** Select only the item moved to; extend from the anchor to it; or keep the selection. */
export type CursorMove = 'select' | 'extend' | 'focus';

export interface Selection {
	paths: ReadonlySet<string>;
	/** Where a Shift range starts. */
	anchor: number | null;
}

/** How far a key moves the cursor, or null for a key that does not move it. */
export function cursorDelta(
	key: string,
	count: number,
	columns: number,
	page: number
): number | null {
	switch (key) {
		case 'ArrowUp':
			return -columns;
		case 'ArrowDown':
			return columns;
		// A list has one column; Left and Right are for the grid.
		case 'ArrowLeft':
			return columns === 1 ? null : -1;
		case 'ArrowRight':
			return columns === 1 ? null : 1;
		case 'Home':
			return -count;
		case 'End':
			return count;
		case 'PageUp':
			return -page;
		case 'PageDown':
			return page;
		default:
			return null;
	}
}

/** Where the cursor lands; with none yet, Up starts at the end and Down at the top. */
export function cursorStep(active: number | null, count: number, delta: number): number | null {
	if (count === 0) return null;
	const last = count - 1;
	if (active === null || active > last) return delta < 0 ? last : 0;
	return Math.min(last, Math.max(0, active + delta));
}

export function cursorMoveFor(event: {
	shiftKey: boolean;
	ctrlKey: boolean;
	metaKey: boolean;
}): CursorMove {
	if (event.shiftKey) return 'extend';
	return event.ctrlKey || event.metaKey ? 'focus' : 'select';
}

/** The selection after the cursor moves to `index`, `how` it moved. */
export function moveCursor(
	paths: readonly string[],
	selection: Selection,
	active: number | null,
	index: number,
	how: CursorMove
): Selection {
	const path = paths[index];
	if (path === undefined || how === 'focus') return selection;
	if (how === 'select') return { paths: new Set([path]), anchor: index };
	const candidate = selection.anchor ?? active;
	const anchor = candidate !== null && candidate < paths.length ? candidate : index;
	const [start, end] = anchor <= index ? [anchor, index] : [index, anchor];
	return { paths: new Set(paths.slice(start, end + 1)), anchor };
}

/** Ctrl+Space, or Ctrl+click: selects or deselects one item, which becomes the anchor. */
export function toggleAt(paths: readonly string[], selection: Selection, index: number): Selection {
	const path = paths[index];
	if (path === undefined) return selection;
	const next = new Set(selection.paths);
	if (!next.delete(path)) next.add(path);
	return { paths: next, anchor: index };
}

/** The first name starting with `prefix`, ignoring case, looking from `from` and wrapping. */
export function findPrefix(names: readonly string[], prefix: string, from: number): number | null {
	const wanted = prefix.toLocaleLowerCase();
	for (let offset = 0; offset < names.length; offset++) {
		const index = (from + offset) % names.length;
		if (names[index]?.toLocaleLowerCase().startsWith(wanted)) return index;
	}
	return null;
}

/** How long a pause ends a typed name. */
const TYPE_AHEAD_PAUSE_MS = 1000;

/** Typing a name selects the first item it starts; the same letter again steps to the next. */
export class TypeAhead {
	private text = '';
	private last = -Infinity;

	/** The index to move to for `typed`, pressed at `now` ms, or null when nothing matches. */
	type(typed: string, now: number, names: readonly string[], active: number | null): number | null {
		const fresh = now - this.last > TYPE_AHEAD_PAUSE_MS;
		if (fresh) this.text = '';
		this.last = now;
		this.text += typed;
		const text = this.text;
		const letters = Array.from(text);
		const repeated = letters.length > 1 && letters.every((letter) => letter === letters[0]);
		const from = active ?? 0;
		// "aaa" steps through the items that start with "a".
		if (repeated && findPrefix(names, text, 0) === null)
			return findPrefix(names, letters[0] ?? '', from + 1);
		// A new name is looked for from the top; a longer one from the current match.
		return findPrefix(names, text, fresh ? 0 : from);
	}
}

/** A key that types a name: one printable character, without Ctrl, Alt or Cmd. */
export function typedCharacter(event: {
	key: string;
	ctrlKey: boolean;
	altKey: boolean;
	metaKey: boolean;
}): string | null {
	if (event.ctrlKey || event.altKey || event.metaKey) return null;
	return Array.from(event.key).length === 1 && event.key.trim() !== '' ? event.key : null;
}
