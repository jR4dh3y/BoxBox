import assert from 'node:assert/strict';
import { test } from 'node:test';
import { setTimeout as delay } from 'node:timers/promises';
import { toastStore } from './toast.svelte';

test('notification waits for its renderer before its visible lifetime starts', async () => {
	toastStore.clear();
	const id = toastStore.success('Lazy notification', 10);
	await delay(30);
	assert.equal(toastStore.toasts[0]?.id, id);
	toastStore.startLifetime(id);
	await delay(30);
	assert.equal(toastStore.toasts.length, 0);
});

test('persistent notification remains dismissible after it becomes visible', async () => {
	const id = toastStore.error('Persistent notification', 0);
	toastStore.startLifetime(id);
	await delay(20);
	assert.equal(toastStore.toasts[0]?.id, id);
	toastStore.remove(id);
	assert.equal(toastStore.toasts.length, 0);
});
