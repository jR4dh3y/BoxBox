import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import type { DesktopTrashItem } from '#lib/api/desktopTrash.js';
import { TrashController, type TrashClient } from './trash.svelte';

function item(overrides: Partial<DesktopTrashItem> = {}): DesktopTrashItem {
	return {
		id: 'a',
		name: 'notes.txt',
		originalPath: 'home/notes.txt',
		deletedAt: '2026-10-01T10:00:00Z',
		isDir: false,
		canRestore: true,
		...overrides
	};
}

function trash(items: DesktopTrashItem[]): TrashClient & { calls: string[] } {
	const calls: string[] = [];
	return {
		calls,
		list: async () => [...items],
		restore: async (id) => {
			calls.push(`restore ${id}`);
			items.splice(
				items.findIndex((entry) => entry.id === id),
				1
			);
		},
		delete: async (id) => {
			calls.push(`delete ${id}`);
			items.splice(
				items.findIndex((entry) => entry.id === id),
				1
			);
		}
	};
}

describe('trash controller', () => {
	test('restores the selected item and reloads the list', async () => {
		const client = trash([item(), item({ id: 'b', name: 'b.txt' })]);
		const controller = new TrashController(client);
		await controller.items.load();
		controller.select('a');

		assert.equal(await controller.run('restore'), true);
		assert.deepEqual(client.calls, ['restore a']);
		assert.deepEqual(
			controller.items.value?.map((entry) => entry.id),
			['b']
		);
		assert.equal(controller.selected, null);
	});

	test('permanent deletion needs a second click', async () => {
		const client = trash([item()]);
		const controller = new TrashController(client);
		await controller.items.load();
		controller.select('a');

		assert.equal(await controller.run('delete'), false);
		assert.equal(controller.confirmingDelete, true);
		assert.deepEqual(client.calls, []);

		assert.equal(await controller.run('delete'), true);
		assert.equal(controller.confirmingDelete, false);
		assert.deepEqual(client.calls, ['delete a']);
	});

	test('selecting another item withdraws the delete confirmation', async () => {
		const controller = new TrashController(trash([item(), item({ id: 'b' })]));
		await controller.items.load();
		controller.select('a');
		await controller.run('delete');
		controller.select('b');
		assert.equal(controller.confirmingDelete, false);
	});

	test('unchecking a selection clears it and withdraws delete confirmation', async () => {
		const client = trash([item()]);
		const controller = new TrashController(client);
		await controller.items.load();
		controller.select('a');
		await controller.run('delete');

		controller.select(null);

		assert.equal(controller.selected, null);
		assert.equal(controller.confirmingDelete, false);
		assert.equal(await controller.run('restore'), false);
		assert.equal(await controller.run('delete'), false);
		assert.deepEqual(client.calls, []);
	});

	test('does not restore an item without a recorded origin', async () => {
		const client = trash([item({ canRestore: false })]);
		const controller = new TrashController(client);
		await controller.items.load();
		controller.select('a');
		assert.equal(await controller.run('restore'), false);
		assert.deepEqual(client.calls, []);
	});
});
