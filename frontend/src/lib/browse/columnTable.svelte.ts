import { settingsStore } from '#lib/stores/settings.svelte.js';
import { toastStore } from '#lib/stores/toast.svelte.js';
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

/** Runs `use` with the column editing code, or says why it could not download. */
function withEdit(use: (edit: ColumnEdit) => void, signal = settingsStore.sessionSignal): void {
	loadEdit().then(
		(edit) => {
			if (!signal.aborted) use(edit);
		},
		() => {
			if (!signal.aborted)
				toastStore.error('Could not load column editing. Check your connection, then reload.');
		}
	);
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
		const signal = settingsStore.sessionSignal;
		if (event.button !== 0 || signal.aborted) return;
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
			if (moved.pointerId !== event.pointerId) return;
			x = moved.clientX - table.getBoundingClientRect().left;
			apply();
		};
		const detach = () => {
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', end);
			window.removeEventListener('pointercancel', end);
			signal.removeEventListener('abort', cancel);
		};
		const cancel = () => {
			detach();
			this.dragging = null;
		};
		const end = (up: PointerEvent) => {
			if (up.pointerId !== event.pointerId) return;
			detach();
			ended = true;
			if (edit) finish();
		};
		signal.addEventListener('abort', cancel, { once: true });
		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', end);
		window.addEventListener('pointercancel', end);
		withEdit((loaded) => {
			edit = loaded;
			apply();
			if (ended) finish();
		}, signal);
	}

	resetWidth(column: Column): void {
		withEdit((edit) => this.save(edit.resetColumnWidth(this.layout, column)));
	}

	toggle(column: Column): void {
		withEdit((edit) => this.save(edit.toggleColumn(this.layout, column)));
	}

	shows(column: Column): boolean {
		return this.specs.some((spec) => spec.column === column);
	}

	private save(layout: ColumnLayout): void {
		settingsStore.set({ ...settingsStore.current, fileColumns: layout });
	}
}
