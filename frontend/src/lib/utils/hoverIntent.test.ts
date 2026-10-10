import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { createHoverIntent } from './hoverIntent';

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe('createHoverIntent', () => {
	test('runs once the pointer has rested on a key', async () => {
		const seen: string[] = [];
		const hover = createHoverIntent((key) => seen.push(key), 10);
		hover('media');
		await wait(40);
		assert.deepEqual(seen, ['media']);
	});

	test('ignores a pointer that sweeps past', async () => {
		const seen: string[] = [];
		const hover = createHoverIntent((key) => seen.push(key), 20);
		hover('a');
		hover('b');
		hover(null);
		await wait(50);
		assert.deepEqual(seen, []);
	});

	test('only the key the pointer ends on runs', async () => {
		const seen: string[] = [];
		const hover = createHoverIntent((key) => seen.push(key), 20);
		hover('a');
		hover('b');
		await wait(50);
		assert.deepEqual(seen, ['b']);
	});
});
