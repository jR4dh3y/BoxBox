import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { flushSync } from 'svelte';
import { createDelayedFlag } from './delayedFlag.svelte';

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function setup(delayMs: number) {
	let source = $state(false);
	let flag!: ReturnType<typeof createDelayedFlag>;
	const stop = $effect.root(() => {
		flag = createDelayedFlag(() => source, delayMs);
	});
	return {
		flag,
		stop,
		set(value: boolean) {
			source = value;
			flushSync();
		}
	};
}

describe('createDelayedFlag', () => {
	test('turns on only after the delay', async () => {
		const { flag, set, stop } = setup(30);
		set(true);
		assert.equal(flag.value, false);
		await wait(60);
		assert.equal(flag.value, true);
		stop();
	});

	test('never turns on for a load that finishes first', async () => {
		const { flag, set, stop } = setup(30);
		set(true);
		await wait(10);
		set(false);
		await wait(60);
		assert.equal(flag.value, false);
		stop();
	});

	test('turns off as soon as the source does', async () => {
		const { flag, set, stop } = setup(10);
		set(true);
		await wait(40);
		set(false);
		assert.equal(flag.value, false);
		stop();
	});
});
