/** Fixed-size rows of `columns` (at least 1) items each; a list is a grid with one column. */
export interface VirtualLayout {
	count: number;
	columns: number;
	/** Height of one row including the gap below it. */
	rowHeight: number;
}

export interface Viewport {
	/** Scroll offset measured from the first row, not from the top of the scroll element. */
	scrollTop: number;
	height: number;
}

/** Items `[start, end)` are rendered; `before` and `after` are the heights the rest would take. */
export interface VirtualRange {
	start: number;
	end: number;
	before: number;
	after: number;
}

/** Columns that fit `width` when each cell is `cellWidth` wide with `gap` between cells. */
export function columnsFor(width: number, cellWidth: number, gap: number): number {
	return Math.max(1, Math.floor((width + gap) / (cellWidth + gap)));
}

export function visibleRange(
	layout: VirtualLayout,
	viewport: Viewport,
	overscanRows: number
): VirtualRange {
	const rows = Math.ceil(layout.count / layout.columns);
	if (rows === 0 || layout.rowHeight <= 0) return { start: 0, end: 0, before: 0, after: 0 };
	const firstRow = Math.max(
		0,
		Math.min(rows - 1, Math.floor(viewport.scrollTop / layout.rowHeight)) - overscanRows
	);
	const lastRow = Math.min(
		rows,
		Math.ceil((viewport.scrollTop + viewport.height) / layout.rowHeight) + overscanRows
	);
	const endRow = Math.max(firstRow + 1, lastRow);
	return {
		start: firstRow * layout.columns,
		end: Math.min(layout.count, endRow * layout.columns),
		before: firstRow * layout.rowHeight,
		after: (rows - endRow) * layout.rowHeight
	};
}

/** The scroll offset that brings item `index` fully into view, or null when it already is. */
export function revealOffset(
	layout: VirtualLayout,
	viewport: Viewport,
	index: number
): number | null {
	const top = Math.floor(index / layout.columns) * layout.rowHeight;
	const bottom = top + layout.rowHeight;
	if (top < viewport.scrollTop) return top;
	if (bottom > viewport.scrollTop + viewport.height) return Math.max(0, bottom - viewport.height);
	return null;
}
