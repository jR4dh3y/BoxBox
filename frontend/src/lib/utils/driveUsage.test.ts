import assert from 'node:assert/strict';
import { test } from 'node:test';
import { DRIVE_USAGE_BAR, driveUsageLevel } from './driveUsage';

test('drives turn amber from 75% and red from 90%', () => {
	assert.equal(driveUsageLevel(0), 'normal');
	assert.equal(driveUsageLevel(74.9), 'normal');
	assert.equal(driveUsageLevel(75), 'warning');
	assert.equal(driveUsageLevel(89.9), 'warning');
	assert.equal(driveUsageLevel(90), 'danger');
});

test('drive bars use fixed status colours, never the accent', () => {
	assert.deepEqual(DRIVE_USAGE_BAR, { normal: 'info', warning: 'warning', danger: 'danger' });
});
