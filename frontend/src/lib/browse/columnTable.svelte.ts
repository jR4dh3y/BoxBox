import { settingsStore } from '#lib/stores/settings.svelte.js';
import {
	columnSpecs,
	fitLayout,
	layoutMinWidth,
	newLayout,
	type Column,
	type ColumnLayout,
	type ColumnSpec
} from './columns.js';

type ColumnEdit = typeof import('./columnEdit.js');
let edit: Promise<ColumnEdit> | null = null;
function loadEdit(): Promise<ColumnEdit> {
	edit ??= import('./columnEdit.js');
	edit.catch(() => {
		edit = null;
	});
	return edit;
}

/**
 * A table's column layout as drawn in its pane: the saved layout fitted to the pane's width,
 * and a border drag that follows the pointer and is saved once the pointer lets go.
 */
export class ColumnTable {
	private paneWidth = $state(0);
	private dragging = $state.raw<ColumnLayout | null>(null);

	readonly layout: ColumnLayout;
	readonly specs: ColumnSpec[];
	readonly minWidth: number;

	constructor() {
		this.layout = $derived.by(() => {
			const layout = settingsStore.current.fileColumns ?? newLayout();
			const shown = this.dragging ?? layout;
			return this.paneWidth > 0 ? fitLayout(shown, this.paneWidth) : shown;
		});
		this.specs = $derived(columnSpecs(this.layout));
		this.minWidth = $derived(layoutMinWidth(this.layout));
	}

	/** Svelte action for the scrolling element the table fills. */
	observe = (node: HTMLElement) => {
		const observer = new ResizeObserver(() => {
			this.paneWidth = node.clientWidth;
		});
		observer.observe(node);
		this.paneWidth = node.clientWidth;
		return { destroy: () => observer.disconnect() };
	};

	/** Loads the editing code early, when the pointer reaches the header. */
	prepare(): void {
		void loadEdit();
	}

	/** Drags `column`'s right border; `table` is the element whose left edge is x = 0. */
	startResize(event: PointerEvent, column: Column, table: HTMLElement): void {
		if (event.button !== 0) return;
		event.preventDefault();
		event.stopPropagation();
		const grip = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
		grip?.setPointerCapture(event.pointerId);
		const start = this.layout;
		// The table widens as the column does; Name is pinned at its width when the drag began.
		const startWidth = table.getBoundingClientRect().width;
		let edit: ColumnEdit | null = null;
		let x: number | null = null;
		let ended = false;
		const apply = () => {
			if (edit && x !== null) this.dragging = edit.resizeColumn(start, column, x, startWidth);
		};
		const finish = () => {
			const done = this.dragging;
			this.dragging = null;
			if (done) this.save(done);
		};
		const move = (moved: PointerEvent) => {
			x = moved.clientX - table.getBoundingClientRect().left;
			apply();
		};
		const end = () => {
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', end);
			window.removeEventListener('pointercancel', end);
			ended = true;
			if (edit) finish();
		};
		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', end);
		window.addEventListener('pointercancel', end);
		void loadEdit().then((loaded) => {
			edit = loaded;
			apply();
			if (ended) finish();
		});
	}

	resetWidth(column: Column): void {
		void loadEdit().then((edit) => this.save(edit.resetColumnWidth(this.layout, column)));
	}

	toggle(column: Column): void {
		void loadEdit().then((edit) => this.save(edit.toggleColumn(this.layout, column)));
	}

	shows(column: Column): boolean {
		return this.specs.some((spec) => spec.column === column);
	}

	private save(layout: ColumnLayout): void {
		settingsStore.set({ ...settingsStore.current, fileColumns: layout });
	}
}
