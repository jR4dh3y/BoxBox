import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
	MIN_COLUMN,
	MIN_NAME,
	columnSpecs,
	fitLayout,
	layoutColumns,
	layoutMinWidth,
	newLayout,
	parseColumnLayout,
	resetColumnWidth,
	resizeColumn,
	toggleColumn
} from './columns.js';

describe('column layout', () => {
	test('Name fills until it is sized', () => {
		const specs = columnSpecs(newLayout());
		assert.deepEqual(
			specs.map((spec) => spec.column),
			['name', 'size', 'type', 'modified']
		);
		assert.deepEqual(specs[0]?.width, { kind: 'fill', min: MIN_NAME });
		assert.deepEqual(specs[3]?.width, { kind: 'fixed', width: 176 });
	});

	test('dragging a border sizes only the column to its left', () => {
		const layout = fitLayout(newLayout(), 1000);
		// 1000px: Name fills 1000 - (96 + 160 + 176) = 568; Size spans 568..664.
		const wider = resizeColumn(layout, 'size', 700, 1000);
		assert.equal(wider.widths.size, 132);
		assert.deepEqual(columnSpecs(wider)[0]?.width, { kind: 'fixed', width: 568 });
		assert.deepEqual(columnSpecs(wider)[2]?.width, { kind: 'fixed', width: 160 });
		const further = resizeColumn(wider, 'size', 900, 1000);
		assert.equal(further.widths.size, 332);
	});

	test('sizing Name hands the spare room to the last column', () => {
		const layout = resizeColumn(newLayout(), 'name', 300, 1000);
		const specs = columnSpecs(layout);
		assert.deepEqual(specs[0]?.width, { kind: 'fixed', width: 300 });
		assert.deepEqual(specs[3]?.width, { kind: 'fill', min: 176 });
		assert.deepEqual(columnSpecs(resetColumnWidth(layout, 'name'))[0]?.width, {
			kind: 'fill',
			min: MIN_NAME
		});
	});

	test('columns keep a minimum and the table its width', () => {
		const layout = resizeColumn(newLayout(), 'type', 0, 1000);
		assert.equal(layout.widths.type, MIN_COLUMN);
		assert.equal(layoutMinWidth(layout), 568 + 96 + MIN_COLUMN + 176);
	});

	test('a narrower pane takes its room from Name', () => {
		const sized = resizeColumn(fitLayout(newLayout(), 1000), 'size', 700, 1000);
		assert.equal(sized.fitted, 1000);
		// A Details panel takes 300px: Name gives them up, the others keep theirs.
		const narrow = fitLayout(sized, 700);
		assert.deepEqual(columnSpecs(narrow)[0]?.width, { kind: 'fixed', width: 268 });
		assert.deepEqual(columnSpecs(narrow)[1]?.width, { kind: 'fixed', width: 132 });
		assert.equal(fitLayout(narrow, 100).widths.name, MIN_NAME);
		assert.deepEqual(fitLayout(sized, 1000), sized);
		assert.deepEqual(columnSpecs(fitLayout(sized, 1400))[0]?.width, {
			kind: 'fixed',
			width: 568
		});
		const typed = resizeColumn(narrow, 'type', 600, 700);
		assert.equal(typed.widths.type, 200);
		assert.equal(typed.fitted, 700);
	});

	test('a layout saved without its pane width fits the pane', () => {
		const saved = parseColumnLayout({
			shown: ['size', 'type', 'modified'],
			widths: { name: 955, size: 94, type: 286 }
		});
		assert.ok(saved);
		assert.equal(fitLayout(saved, 1200).widths.name, 644);
		assert.equal(fitLayout(saved, 1600).widths.name, 955);
	});

	test('hiding and showing keeps the table’s order', () => {
		const hidden = toggleColumn(newLayout(), 'size');
		assert.deepEqual(layoutColumns(hidden), ['name', 'type', 'modified']);
		assert.deepEqual(layoutColumns(toggleColumn(hidden, 'size')), [
			'name',
			'size',
			'type',
			'modified'
		]);
	});

	test('a saved column this version does not know is not drawn', () => {
		const layout = parseColumnLayout({ shown: ['size'], widths: {} });
		assert.ok(layout);
		assert.deepEqual(layoutColumns(layout), ['name', 'size']);
	});

	test('an unreadable layout falls back', () => {
		assert.equal(parseColumnLayout({ shown: ['bogus'], widths: {} }), null);
		assert.equal(parseColumnLayout({ shown: [], widths: { name: 'wide' } }), null);
		assert.equal(parseColumnLayout('x'), null);
	});
});
