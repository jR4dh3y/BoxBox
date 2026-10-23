import { tick } from 'svelte';
import { columnsFor, revealOffset, visibleRange, type VirtualLayout } from './virtualWindow';

export interface VirtualScrollOptions {
	count: number;
	/** Cell width, gap and horizontal padding of a grid; a list has one column. */
	cell?: { width: number; gap: number; padding: number };
	/** Gap between rows, added to the measured item height. */
	rowGap: number;
	/** Pixels of content above the first row, such as a table header or grid padding. */
	offsetTop: number;
	/** Pixels at the top of the viewport covered by sticky content. */
	stickyTop: number;
	/** Row height until a rendered item has been measured. */
	estimatedItemHeight: number;
}

/**
 * Renders only the rows in view. Items share one height, measured from whichever item is
 * rendered, so the full collection costs one array and two spacers, not one element per item.
 */
export function createVirtualScroll(options: () => VirtualScrollOptions) {
	let element = $state<HTMLElement | null>(null);
	let scrollTop = $state(0);
	let viewportHeight = $state(0);
	let contentWidth = $state(0);
	let itemHeight = $state(0);

	const layout = $derived.by<VirtualLayout>(() => {
		const { count, cell, rowGap, estimatedItemHeight } = options();
		return {
			count,
			columns: cell ? columnsFor(contentWidth - 2 * cell.padding, cell.width, cell.gap) : 1,
			rowHeight: (itemHeight || estimatedItemHeight) + rowGap
		};
	});
	const viewport = $derived.by(() => {
		const { offsetTop, stickyTop } = options();
		return { scrollTop: scrollTop - offsetTop + stickyTop, height: viewportHeight - stickyTop };
	});
	const range = $derived(
		visibleRange(layout, viewport, Math.max(1, Math.ceil(5 / layout.columns)))
	);

	function readViewport(node: HTMLElement): void {
		scrollTop = node.scrollTop;
		viewportHeight = node.clientHeight;
		contentWidth = node.clientWidth;
	}

	function reveal(index: number): void {
		const node = element;
		if (!node) return;
		const offset = revealOffset(layout, viewport, index);
		if (offset === null) return;
		const { offsetTop, stickyTop } = options();
		node.scrollTop = offset + offsetTop - stickyTop;
		scrollTop = node.scrollTop;
	}

	const itemObserver = new ResizeObserver((entries) => {
		const height = entries.at(-1)?.target.getBoundingClientRect().height;
		if (height) itemHeight = height;
	});
	// The element that last received focus inside the scroller.
	let focused: EventTarget | null = null;

	return {
		get range() {
			return range;
		},
		get columns() {
			return layout.columns;
		},
		/** Attach to the scroll container; give it tabindex="-1" so it can hold focus for an item. */
		scroller(node: HTMLElement) {
			element = node;
			readViewport(node);
			const onScroll = () => (scrollTop = node.scrollTop);
			// The scroller taking focus for an unmounted item must not forget that item.
			const onFocusIn = (event: FocusEvent) => {
				if (event.target !== node) focused = event.target;
			};
			node.addEventListener('scroll', onScroll, { passive: true });
			node.addEventListener('focusin', onFocusIn);
			const observer = new ResizeObserver(() => readViewport(node));
			observer.observe(node);
			return () => {
				node.removeEventListener('scroll', onScroll);
				node.removeEventListener('focusin', onFocusIn);
				observer.disconnect();
				itemObserver.disconnect();
				element = null;
			};
		},
		/**
		 * Attach to each rendered item; the shared height follows the latest one to resize. A focused
		 * item that scrolls out hands focus to the scroller, so the keyboard keeps working.
		 */
		item(node: HTMLElement) {
			itemObserver.observe(node);
			return () => {
				itemObserver.unobserve(node);
				// Teardown runs after the node is detached, so focus has already fallen to the body.
				const lost = document.activeElement === null || document.activeElement === document.body;
				if (lost && focused instanceof Node && node.contains(focused))
					element?.focus({ preventScroll: true });
			};
		},
		/** Path of the item that last held focus, even after it scrolled out or moved in a re-sort. */
		get focusedPath() {
			return focused instanceof Element
				? (focused.closest('[data-file-path]')?.getAttribute('data-file-path') ?? null)
				: null;
		},
		/** True once the scroll container is attached and `reveal` can scroll it. */
		get ready() {
			return element !== null;
		},
		/** Scrolls item `index` into view if it is not already. */
		reveal,
		/** Scrolls item `index` into view, waits for it to render, and focuses it. */
		async focus(index: number): Promise<void> {
			const node = element;
			if (!node) return;
			reveal(index);
			await tick();
			node
				.querySelector<HTMLElement>(`[data-file-index="${index}"]`)
				?.focus({ preventScroll: true });
		}
	};
}

export type VirtualScroll = ReturnType<typeof createVirtualScroll>;
