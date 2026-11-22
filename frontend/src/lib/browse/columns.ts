/**
 * The columns of the folder list and their widths, as the desktop app and Thunar keep them:
 * dragging a border sizes the column to its left and moves the ones after it, and the table
 * scrolls sideways when the columns are wider than the pane. Name fills the room left until
 * it is sized by hand; then the last column does.
 */
import { isRecord } from '#lib/api/parse.js';

export type Column = 'name' | 'size' | 'type' | 'modified';

const LABELS: Record<Column, string> = {
	name: 'Name',
	size: 'Size',
	type: 'Type',
	modified: 'Modified'
};

const DEFAULT_WIDTHS: Record<Column, number> = {
	name: 320,
	size: 96,
	type: 160,
	modified: 176
};

export const MIN_COLUMN = 48;
export const MIN_NAME = 120;

/** The columns shown after Name, in Thunar's order. */
export const AVAILABLE_COLUMNS: readonly Column[] = ['size', 'type', 'modified'];

export function columnLabel(column: Column): string {
	return LABELS[column];
}

/** Sizes line up on the right, as numbers do. */
export function rightAligned(column: Column): boolean {
	return column === 'size';
}

function minWidth(column: Column): number {
	return column === 'name' ? MIN_NAME : MIN_COLUMN;
}

/** Which columns a table shows after Name, and the widths dragged by hand. */
export interface ColumnLayout {
	shown: readonly Column[];
	/** Columns sized by hand; others keep their default width, and Name fills. */
	widths: Readonly<Partial<Record<Column, number>>>;
	/**
	 * How wide the pane was when a column was last sized. In a narrower pane Name gives up
	 * the difference, so a Details panel moves the columns over instead of hiding them.
	 */
	fitted: number | null;
}

/** A column's width as drawn: fixed, or taking the room the others leave, at least `min`. */
export type Width = { kind: 'fixed'; width: number } | { kind: 'fill'; min: number };

export interface ColumnSpec {
	column: Column;
	width: Width;
}

export function newLayout(): ColumnLayout {
	return { shown: AVAILABLE_COLUMNS, widths: {}, fitted: null };
}

function width(layout: ColumnLayout, column: Column): number {
	const sized = layout.widths[column];
	const value = sized !== undefined && Number.isFinite(sized) ? sized : DEFAULT_WIDTHS[column];
	return Math.max(value, minWidth(column));
}

function drawn(spec: ColumnSpec): number {
	return spec.width.kind === 'fixed' ? spec.width.width : spec.width.min;
}

/** Name, then the shown columns this table has, each once. */
export function layoutColumns(layout: ColumnLayout): Column[] {
	const columns: Column[] = ['name'];
	for (const column of layout.shown) {
		if (AVAILABLE_COLUMNS.includes(column) && !columns.includes(column)) columns.push(column);
	}
	return columns;
}

/** How wide each column is drawn, Name first. */
export function columnSpecs(layout: ColumnLayout): ColumnSpec[] {
	const columns = layoutColumns(layout);
	const last = columns.length - 1;
	const nameSized = layout.widths.name !== undefined;
	return columns.map((column, index): ColumnSpec => {
		const value = width(layout, column);
		if (column === 'name' && !nameSized) return { column, width: { kind: 'fill', min: MIN_NAME } };
		// Only Name: it is the whole row.
		if (column === 'name' && index === last) return { column, width: { kind: 'fill', min: value } };
		if (index === last && nameSized) return { column, width: { kind: 'fill', min: value } };
		return { column, width: { kind: 'fixed', width: value } };
	});
}

/**
 * The layout as drawn in a pane `pane` pixels wide: a sized Name narrower by the width the
 * pane lost since it was sized, down to its minimum.
 */
export function fitLayout(layout: ColumnLayout, pane: number): ColumnLayout {
	const name = layout.widths.name;
	if (name === undefined) return { ...layout, fitted: pane };
	const fitted =
		layout.fitted ?? layoutColumns(layout).reduce((sum, column) => sum + width(layout, column), 0);
	const lost = Math.max(fitted - pane, 0);
	return {
		...layout,
		widths: { ...layout.widths, name: Math.round(Math.max(name - lost, MIN_NAME)) },
		fitted: pane
	};
}

/** The layout with `column`'s right border dragged to `x` pixels from the table's left. */
export function resizeColumn(
	layout: ColumnLayout,
	column: Column,
	x: number,
	tableWidth: number
): ColumnLayout {
	const widths = { ...layout.widths };
	// Sizing another column pins Name where it is drawn, so a wider column does not narrow
	// Name and slide away from the pointer. The last column fills from then on.
	if (column !== 'name' && widths.name === undefined) {
		const fixed = columnSpecs(layout).reduce(
			(sum, spec) => sum + (spec.width.kind === 'fixed' ? spec.width.width : 0),
			0
		);
		widths.name = Math.round(Math.max(tableWidth - fixed, MIN_NAME));
	}
	const pinned: ColumnLayout = { ...layout, widths, fitted: layout.fitted ?? tableWidth };
	let left = 0;
	for (const spec of columnSpecs(pinned)) {
		if (spec.column === column) break;
		left += drawn(spec);
	}
	return {
		...pinned,
		widths: { ...widths, [column]: Math.round(Math.max(x - left, minWidth(column))) }
	};
}

/** The layout with `column` back at its default width; Name fills again. */
export function resetColumnWidth(layout: ColumnLayout, column: Column): ColumnLayout {
	const widths = { ...layout.widths };
	delete widths[column];
	return { ...layout, widths };
}

/** The layout with `column` shown or hidden, in the table's order. */
export function toggleColumn(layout: ColumnLayout, column: Column): ColumnLayout {
	if (layout.shown.includes(column))
		return { ...layout, shown: layout.shown.filter((shown) => shown !== column) };
	return {
		...layout,
		shown: [...layout.shown, column].sort(
			(a, b) => AVAILABLE_COLUMNS.indexOf(a) - AVAILABLE_COLUMNS.indexOf(b)
		)
	};
}

/** The narrowest the table is drawn before it scrolls sideways. */
export function layoutMinWidth(layout: ColumnLayout): number {
	return columnSpecs(layout).reduce((sum, spec) => sum + drawn(spec), 0);
}

function isColumn(value: unknown): value is Column {
	return typeof value === 'string' && Object.hasOwn(LABELS, value);
}

/** A saved layout, or null when it cannot be read, so a bad entry loses only itself. */
export function parseColumnLayout(value: unknown): ColumnLayout | null {
	if (!isRecord(value) || !Array.isArray(value.shown) || !isRecord(value.widths)) return null;
	if (!value.shown.every(isColumn)) return null;
	const widths: Partial<Record<Column, number>> = {};
	for (const [column, saved] of Object.entries(value.widths)) {
		if (!isColumn(column) || typeof saved !== 'number' || !Number.isFinite(saved)) return null;
		widths[column] = saved;
	}
	const fitted = typeof value.fitted === 'number' ? value.fitted : null;
	return { shown: value.shown, widths, fitted };
}
