import { SvelteSet } from 'svelte/reactivity';
import type { FileInfo } from '#lib/api/files.js';
import {
	TypeAhead,
	cursorDelta,
	cursorMoveFor,
	cursorStep,
	moveCursor,
	toggleAt,
	typedCharacter,
	type Selection
} from './cursor.js';

export interface ViewCursorProps {
	items: readonly FileInfo[];
	selectedPaths: ReadonlySet<string>;
	onSelectionChange?: (paths: Set<string>) => void;
}

/** How a click changed the selection; a plain click may also open the item. */
export type ClickResult = 'replaced' | 'changed';

/** What a key did: nothing of the cursor's, moved it to `index`, or was used without a move. */
export type KeyResult = { kind: 'ignored' } | { kind: 'moved'; index: number } | { kind: 'used' };

/**
 * The keyboard cursor and the selection gestures the list and the grid share: arrows, page
 * keys and type-ahead move the cursor, clicks and Ctrl+Space pick items, and Shift ranges
 * start from the anchor the last plain pick left.
 */
export class ViewCursor {
	/** The item under the keyboard cursor. */
	activePath = $state<string | null>(null);
	private anchor: number | null = null;
	private readonly typing = new TypeAhead();
	private readonly props: () => ViewCursorProps;

	constructor(props: () => ViewCursorProps) {
		this.props = props;
	}

	get activeIndex(): number {
		const path = this.activePath;
		return path === null ? -1 : this.props().items.findIndex((item) => item.path === path);
	}

	private apply(next: Selection): void {
		this.anchor = next.anchor;
		this.props().onSelectionChange?.(new SvelteSet(next.paths));
	}

	private selection(): Selection {
		return { paths: this.props().selectedPaths, anchor: this.anchor };
	}

	private paths(): string[] {
		return this.props().items.map((item) => item.path);
	}

	/** A click: plain replaces, Ctrl toggles, Shift selects a range, Ctrl+Shift adds one. */
	click(
		item: FileInfo,
		event: { ctrlKey: boolean; metaKey: boolean; shiftKey: boolean }
	): ClickResult {
		const paths = this.paths();
		const index = paths.indexOf(item.path);
		const active = this.activeIndex;
		this.activePath = item.path;
		if (index < 0) return 'changed';
		const command = event.ctrlKey || event.metaKey;
		if (event.shiftKey) {
			const range = moveCursor(
				paths,
				this.selection(),
				active < 0 ? null : active,
				index,
				'extend'
			);
			this.apply(
				command
					? {
							paths: new SvelteSet([...this.props().selectedPaths, ...range.paths]),
							anchor: range.anchor
						}
					: range
			);
			return 'changed';
		}
		if (command) {
			this.apply(toggleAt(paths, this.selection(), index));
			return 'changed';
		}
		this.apply(moveCursor(paths, this.selection(), null, index, 'select'));
		return 'replaced';
	}

	/** A checkbox or Space: picks or drops one item without touching the rest. */
	toggle(item: FileInfo): void {
		const index = this.paths().indexOf(item.path);
		this.activePath = item.path;
		if (index >= 0) this.apply(toggleAt(this.paths(), this.selection(), index));
	}

	/** Puts the cursor on `item`, as a right-click does, selecting it unless it already is. */
	point(item: FileInfo): void {
		this.activePath = item.path;
		if (!this.props().selectedPaths.has(item.path)) {
			const index = this.paths().indexOf(item.path);
			this.apply({ paths: new SvelteSet([item.path]), anchor: index < 0 ? null : index });
		}
	}

	/** Moves for a navigation, toggle or typed key. */
	key(
		event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>,
		layout: { columns: number; page: number },
		now: number
	): KeyResult {
		const { items } = this.props();
		const paths = this.paths();
		const active = this.activeIndex < 0 ? null : this.activeIndex;
		if (event.altKey) return { kind: 'ignored' };
		if (event.key === ' ' && !event.shiftKey) {
			if (active === null) return { kind: 'ignored' };
			this.apply(toggleAt(paths, this.selection(), active));
			return { kind: 'moved', index: active };
		}
		const delta = cursorDelta(event.key, items.length, layout.columns, layout.page);
		if (delta !== null) {
			const index = cursorStep(active, items.length, delta);
			if (index === null) return { kind: 'ignored' };
			this.apply(moveCursor(paths, this.selection(), active, index, cursorMoveFor(event)));
			this.activePath = paths[index] ?? null;
			return { kind: 'moved', index };
		}
		const typed = typedCharacter(event);
		if (typed === null) return { kind: 'ignored' };
		const index = this.typing.type(
			typed,
			now,
			items.map((item) => item.name),
			active
		);
		if (index === null) return { kind: 'used' };
		this.apply(moveCursor(paths, this.selection(), active, index, 'select'));
		this.activePath = paths[index] ?? null;
		return { kind: 'moved', index };
	}

	/** The selection when there is one, else the item under the cursor. */
	targets(): FileInfo[] {
		const { items, selectedPaths } = this.props();
		if (selectedPaths.size > 0) return items.filter((item) => selectedPaths.has(item.path));
		const active = items[this.activeIndex];
		return active ? [active] : [];
	}
}
