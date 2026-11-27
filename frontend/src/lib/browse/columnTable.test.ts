import assert from 'node:assert/strict';
import { test } from 'node:test';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { ColumnTable } from './columnTable.svelte.js';

test('a queued column toggle stops when its preferences session ends', async () => {
	settingsStore.clearSession();
	try {
		const table = new ColumnTable();
		const nextTurn = () => new Promise((resolve) => setTimeout(resolve, 0));
		await import('./columnEdit.js');

		table.toggle('type');
		await nextTurn();
		assert.ok(settingsStore.current.fileColumns, 'a toggle in the active session saves');

		table.toggle('type');
		settingsStore.clearSession();
		const fresh = settingsStore.current;
		await nextTurn();
		assert.equal(settingsStore.current, fresh);
	} finally {
		settingsStore.clearSession();
	}
});
