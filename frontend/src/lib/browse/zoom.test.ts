import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { DEFAULT_ZOOM, GRID_STEPS, WheelZoom, gridMetrics, listMetrics, zoomStep } from './zoom.js';

describe('zoom', () => {
	test('steps stay within the view, and reset returns to the original size', () => {
		assert.equal(zoomStep(1, 'in', 4), 2);
		assert.equal(zoomStep(3, 'in', 4), 3);
		assert.equal(zoomStep(0, 'out', 4), 0);
		assert.equal(zoomStep(3, 'reset', 4), DEFAULT_ZOOM);
		assert.equal(listMetrics(DEFAULT_ZOOM).rowHeight, 32);
		assert.equal(listMetrics(0).rowHeight, 28, 'step 0 is the old compact list');
		assert.equal(gridMetrics(99), GRID_STEPS.at(-1));
	});

	test('a mouse notch is one step; a touchpad steps every 40 pixels', () => {
		const wheel = new WheelZoom();
		assert.equal(wheel.steps(-100, 0), 1);
		assert.equal(wheel.steps(3, 1), -1);
		assert.equal(wheel.steps(-30, 0), 0);
		assert.equal(wheel.steps(-30, 0), 1);
		assert.equal(wheel.steps(10, 0), 0);
	});
});
