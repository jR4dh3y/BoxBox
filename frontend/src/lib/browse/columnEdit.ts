/** Editing a column layout, loaded with the first border drag or column toggle. */
import {
	AVAILABLE_COLUMNS,
	MIN_NAME,
	columnSpecs,
	drawn,
	minWidth,
	type Column,
	type ColumnLayout
} from './columns.js';

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
