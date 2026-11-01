import assert from 'node:assert/strict';
import { test } from 'node:test';
import { coalesceRefresh } from './desktopWatch';

test('a burst of changes runs one refresh now and one after it, never two at once', async () => {
	let calls = 0;
	let inFlight = 0;
	let maxInFlight = 0;
	const finishers: Array<() => void> = [];
	const refresh = coalesceRefresh(() => {
		calls++;
		inFlight++;
		maxInFlight = Math.max(maxInFlight, inFlight);
		return new Promise<void>((resolve) =>
			finishers.push(() => {
				inFlight--;
				resolve();
			})
		);
	});

	for (let change = 0; change < 20; change++) refresh();
	assert.equal(calls, 1);

	finishers.shift()?.();
	await new Promise((resolve) => setTimeout(resolve, 0));
	assert.equal(calls, 2, 'changes during the first refresh need one more');

	finishers.shift()?.();
	await new Promise((resolve) => setTimeout(resolve, 0));
	assert.equal(calls, 2);
	assert.equal(maxInFlight, 1);
});

test('a failed refresh does not stop later ones', async () => {
	let calls = 0;
	const refresh = coalesceRefresh(async () => {
		calls++;
		throw new Error('offline');
	});
	refresh();
	await new Promise((resolve) => setTimeout(resolve, 0));
	refresh();
	await new Promise((resolve) => setTimeout(resolve, 0));
	assert.equal(calls, 2);
});
