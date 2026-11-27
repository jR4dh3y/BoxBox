import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { SpringLoad, dragScrollStep } from './springLoad.js';

const tick = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe('spring-loaded folders', () => {
	test('a drag that rests on a target opens it once', async () => {
		const opened: string[] = [];
		const spring = new SpringLoad(10);
		spring.rest('a', () => opened.push('a'));
		spring.rest('a', () => opened.push('a'));
		await tick(25);
		assert.deepEqual(opened, ['a']);
	});

	test('moving on or leaving starts the wait again', async () => {
		const opened: string[] = [];
		const spring = new SpringLoad(10);
		spring.rest('a', () => opened.push('a'));
		spring.rest('b', () => opened.push('b'));
		await tick(25);
		spring.rest('c', () => opened.push('c'));
		spring.cancel();
		await tick(25);
		assert.deepEqual(opened, ['b']);
	});

	test('a drag scrolls near the edges, faster deeper in', () => {
		assert.equal(dragScrollStep(300, 100, 500), 0);
		assert.ok(dragScrollStep(130, 100, 500) < 0);
		assert.ok(dragScrollStep(101, 100, 500) < dragScrollStep(130, 100, 500));
		assert.equal(dragScrollStep(500, 100, 500), 16);
	});
});
