import { untrack } from 'svelte';
import { recallPosition, rememberPosition, type ScrollAnchor } from './fixedRowPositions';

interface FixedRowWindowOptions<T> {
	items: () => readonly T[];
	itemKey: (item: T) => string;
	viewKey: () => string;
	isReady: () => boolean;
	rowHeight: () => number;
	itemOffset: () => number;
	viewportOffset?: () => number;
	contentEndPadding?: () => number;
	columnsForWidth?: (width: number) => number;
}

interface RowGeometry {
	columns: number;
	rowHeight: number;
	itemOffset: number;
	viewportOffset: number;
}

export function createFixedRowWindow<T>(options: FixedRowWindowOptions<T>) {
	let scrollTop = $state(0);
	let viewportHeight = $state(0);
	let viewportWidth = $state(0);
	let attached = $state(false);
	let element: HTMLElement | null = null;
	let activeViewKey = options.viewKey();
	let restorePending = true;
	let lastItems: readonly T[] | null = null;
	let lastGeometry: RowGeometry | null = null;

	const itemCount = $derived(options.items().length);
	const columnCount = $derived.by(() => {
		if (!options.columnsForWidth) return 1;
		return Math.max(1, Math.floor(options.columnsForWidth(viewportWidth)));
	});
	const rowCount = $derived(Math.ceil(itemCount / columnCount));
	const firstVisibleRow = $derived.by(() => {
		const height = Math.max(1, options.rowHeight());
		const top = Math.max(0, scrollTop + (options.viewportOffset?.() ?? 0) - options.itemOffset());
		return Math.min(rowCount, Math.floor(top / height));
	});
	const lastVisibleRow = $derived.by(() => {
		const height = Math.max(1, options.rowHeight());
		const bottom = Math.max(0, scrollTop + viewportHeight - options.itemOffset());
		return Math.min(rowCount, Math.max(firstVisibleRow, Math.ceil(bottom / height)));
	});
	// About five items beyond each edge: five list rows, but one row of a wide grid, so a grid
	// stays under the same element budget as the list.
	const overscan = $derived(Math.max(1, Math.ceil(5 / columnCount)));
	const firstRow = $derived(Math.max(0, firstVisibleRow - overscan));
	const lastRow = $derived(Math.min(rowCount, lastVisibleRow + overscan));
	const visibleRows = $derived.by(() => {
		const rows: number[] = [];
		for (let row = firstRow; row < lastRow; row += 1) rows.push(row);
		return rows;
	});
	const itemStart = $derived(firstRow * columnCount);
	const itemEnd = $derived(Math.min(itemCount, lastRow * columnCount));

	function geometry(): RowGeometry {
		return {
			columns: columnCount,
			rowHeight: Math.max(1, options.rowHeight()),
			itemOffset: options.itemOffset(),
			viewportOffset: options.viewportOffset?.() ?? 0
		};
	}

	function captureAnchor(items: readonly T[], metrics: RowGeometry): ScrollAnchor {
		const currentScrollTop = element?.scrollTop ?? scrollTop;
		const row = Math.max(
			0,
			Math.floor(
				Math.max(0, currentScrollTop + metrics.viewportOffset - metrics.itemOffset) /
					metrics.rowHeight
			)
		);
		const itemIndex = Math.min(row * metrics.columns, Math.max(0, items.length - 1));
		const item = items[itemIndex];
		return {
			itemKey: item === undefined ? null : options.itemKey(item),
			itemIndex,
			offset:
				currentScrollTop + metrics.viewportOffset - (metrics.itemOffset + row * metrics.rowHeight)
		};
	}

	function savePosition(key: string, items: readonly T[], metrics: RowGeometry) {
		const anchor = captureAnchor(items, metrics);
		rememberPosition(key, anchor.itemKey, anchor.itemIndex, anchor.offset);
	}

	function applyAnchor(anchor: ScrollAnchor, items: readonly T[], metrics: RowGeometry) {
		if (!element || items.length === 0) {
			setScrollTop(0);
			return;
		}

		let itemIndex = -1;
		if (anchor.itemKey !== null) {
			for (let index = 0; index < items.length; index += 1) {
				const item = items[index];
				if (item !== undefined && options.itemKey(item) === anchor.itemKey) {
					itemIndex = index;
					break;
				}
			}
		}
		if (itemIndex < 0) {
			itemIndex = Math.min(items.length - 1, Math.max(0, anchor.itemIndex));
		}

		const row = Math.floor(itemIndex / metrics.columns);
		const offset = Math.max(-metrics.rowHeight, Math.min(metrics.rowHeight - 1, anchor.offset));
		setScrollTop(metrics.itemOffset + row * metrics.rowHeight + offset - metrics.viewportOffset);
	}

	function setScrollTop(value: number) {
		if (!element) return;
		const totalHeight =
			options.itemOffset() + rowCount * options.rowHeight() + (options.contentEndPadding?.() ?? 0);
		const maxScrollTop = Math.max(0, totalHeight - element.clientHeight);
		const nextScrollTop = Math.min(maxScrollTop, Math.max(0, value));
		element.scrollTop = nextScrollTop;
		scrollTop = element.scrollTop;
	}

	function onScroll() {
		if (!element) return;
		scrollTop = element.scrollTop;
		if (options.isReady()) savePosition(activeViewKey, options.items(), geometry());
	}

	function scrollToItem(index: number) {
		const items = options.items();
		if (!element || items.length === 0) return;
		const safeIndex = Math.min(items.length - 1, Math.max(0, Math.trunc(index)));
		const metrics = geometry();
		const row = Math.floor(safeIndex / metrics.columns);
		const itemTop = metrics.itemOffset + row * metrics.rowHeight;
		const itemBottom = itemTop + metrics.rowHeight;
		const viewportTop = element.scrollTop + metrics.viewportOffset;
		const viewportBottom = element.scrollTop + element.clientHeight;
		let nextScrollTop = element.scrollTop;

		if (itemTop < viewportTop) {
			nextScrollTop = itemTop - metrics.viewportOffset;
		} else if (itemBottom > viewportBottom) {
			nextScrollTop = itemBottom - element.clientHeight;
		}

		setScrollTop(nextScrollTop);
		onScroll();
	}

	function attach(node: HTMLElement) {
		element = node;
		attached = true;
		viewportHeight = node.clientHeight;
		viewportWidth = node.clientWidth;
		scrollTop = node.scrollTop;

		const observer = new ResizeObserver((entries) => {
			for (const entry of entries) {
				if (entry.target !== node) continue;
				viewportHeight = entry.contentRect.height;
				viewportWidth = entry.contentRect.width;
			}
		});
		observer.observe(node);

		return {
			destroy() {
				observer.disconnect();
				if (element === node) {
					element = null;
					attached = false;
				}
			}
		};
	}

	$effect(() => {
		const viewKey = options.viewKey();
		const ready = options.isReady();
		const items = options.items();
		const metrics = geometry();
		const isAttached = attached;

		untrack(() => {
			if (viewKey !== activeViewKey) {
				activeViewKey = viewKey;
				restorePending = true;
			}
			if (!element || !isAttached || !ready) return;

			if (restorePending) {
				const saved = recallPosition(viewKey);
				if (saved) applyAnchor(saved, items, metrics);
				else setScrollTop(0);
				restorePending = false;
			} else if (
				lastItems !== items ||
				lastGeometry === null ||
				lastGeometry.columns !== metrics.columns ||
				lastGeometry.rowHeight !== metrics.rowHeight ||
				lastGeometry.itemOffset !== metrics.itemOffset ||
				lastGeometry.viewportOffset !== metrics.viewportOffset
			) {
				const saved = recallPosition(viewKey) ?? captureAnchor(items, lastGeometry ?? metrics);
				applyAnchor(saved, items, metrics);
			}

			lastItems = items;
			lastGeometry = metrics;
			savePosition(viewKey, items, metrics);
		});
	});

	return {
		get rowCount() {
			return rowCount;
		},
		get columnCount() {
			return columnCount;
		},
		get firstVisibleRow() {
			return firstVisibleRow;
		},
		get lastVisibleRow() {
			return lastVisibleRow;
		},
		get visibleRowCount() {
			return Math.max(1, lastVisibleRow - firstVisibleRow);
		},
		get visibleRows() {
			return visibleRows;
		},
		get itemStart() {
			return itemStart;
		},
		get itemEnd() {
			return itemEnd;
		},
		get beforeSize() {
			return firstRow * options.rowHeight();
		},
		get afterSize() {
			return Math.max(0, rowCount - lastRow) * options.rowHeight();
		},
		onScroll,
		scrollToItem,
		attach
	};
}
