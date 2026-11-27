/**
 * Rubber-band selection, as in Nautilus, Thunar and the desktop app: a drag that starts on
 * empty space selects every item the rectangle touches. Rows and tiles are uniform, so the
 * items under the rectangle come from arithmetic, not from element bounds.
 */
import type { GridMetrics } from './zoom.js';

export interface Point {
	x: number;
	y: number;
}

/** Where items are, in content coordinates of the items' area. */
export type BandLayout =
	{ kind: 'list'; rowHeight: number } | { kind: 'grid'; metrics: GridMetrics; columns: number };

/** How far the pointer moves before a press becomes a drag. */
export const BAND_THRESHOLD = 4;
/** The most a held drag scrolls per frame past the pane's edge. */
export const BAND_MAX_SCROLL_STEP = 24;

/** Items a rectangle between two corners touches, in order. */
export function itemsInBand(layout: BandLayout, a: Point, b: Point, count: number): number[] {
	const left = Math.min(a.x, b.x);
	const right = Math.max(a.x, b.x);
	const top = Math.min(a.y, b.y);
	const bottom = Math.max(a.y, b.y);
	if (bottom < 0 || count === 0) return [];
	const rows = (height: number, total: number): number[] => {
		const first = Math.floor(Math.max(top, 0) / height);
		const last = Math.min(Math.floor(bottom / height), total - 1);
		const list: number[] = [];
		for (let row = first; row <= last; row++) list.push(row);
		return list;
	};
	if (layout.kind === 'list') return rows(layout.rowHeight, count).filter((index) => index < count);
	const { metrics, columns } = layout;
	const tileHeight = metrics.rowHeight - metrics.padding;
	const found: number[] = [];
	for (const row of rows(metrics.rowHeight, Math.ceil(count / columns))) {
		// The row's top padding is folded into every row.
		const tileTop = row * metrics.rowHeight + metrics.padding;
		if (bottom < tileTop || top > tileTop + tileHeight) continue;
		for (let column = 0; column < columns; column++) {
			const tileLeft = metrics.padding + column * (metrics.tileWidth + metrics.gap);
			const index = row * columns + column;
			if (index < count && right >= tileLeft && left <= tileLeft + metrics.tileWidth)
				found.push(index);
		}
	}
	return found;
}

/** The item under a point, if any. */
export function itemAt(layout: BandLayout, at: Point, count: number): number | null {
	return itemsInBand(layout, at, at, count)[0] ?? null;
}

/** How far to scroll this frame while the pointer is held `past` pixels beyond an edge. */
export function edgeScrollStep(pointerY: number, top: number, bottom: number): number {
	const past = pointerY < top ? pointerY - top : pointerY > bottom ? pointerY - bottom : 0;
	return Math.max(-BAND_MAX_SCROLL_STEP, Math.min(BAND_MAX_SCROLL_STEP, past));
}
