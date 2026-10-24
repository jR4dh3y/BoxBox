import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import type { DesktopServer } from '#lib/api/desktopServers.js';
import { ServersController, type ServersClient } from './servers.svelte';

function client(servers: DesktopServer[]): ServersClient & { calls: string[] } {
	const calls: string[] = [];
	return {
		calls,
		list: async () => [...servers],
		save: async (command) => {
			calls.push(`save ${command.kind} ${command.name}`);
			const saved = {
				id: command.kind === 'update' ? command.id : 'new',
				name: command.name,
				url:
					command.kind === 'create'
						? command.url
						: (servers.find((server) => server.id === command.id)?.url ?? ''),
				expiresAt: '2027-01-01'
			};
			const index = servers.findIndex((server) => server.id === saved.id);
			if (index >= 0) servers[index] = saved;
			else servers.push(saved);
			return saved;
		},
		remove: async (id) => {
			calls.push(`remove ${id}`);
			servers.splice(
				servers.findIndex((server) => server.id === id),
				1
			);
		},
		pair: async (id, code) => {
			calls.push(`pair ${id} ${code}`);
			return servers.find((server) => server.id === id) ?? home;
		}
	};
}

const home = { id: 'home', name: 'Home', url: 'https://home.test/', expiresAt: '2027-01-01' };

describe('saved servers', () => {
	test('adding a server closes the editor and shows the saved list', async () => {
		const controller = new ServersController(client([]));
		controller.edit({ kind: 'create' });
		controller.draft = { name: 'Office', url: 'https://office.test', code: 'ABCD-EFGH' };
		assert.equal(await controller.save(), true);
		assert.equal(controller.editor.kind, 'closed');
		assert.deepEqual(
			controller.servers.value?.map((server) => server.name),
			['Office']
		);
	});

	test('a failed save keeps the editor and what the user typed', async () => {
		const failing = client([]);
		failing.save = async () => {
			throw new Error('A connection with that name already exists');
		};
		const controller = new ServersController(failing);
		controller.edit({ kind: 'create' });
		controller.draft = { name: 'Home', url: 'https://home.test', code: 'ABCD-EFGH' };
		assert.equal(await controller.save(), false);
		assert.equal(controller.editor.kind, 'create');
		assert.equal(controller.draft.name, 'Home');
		assert.equal(controller.action.error, 'A connection with that name already exists');
	});

	test('removing asks once more on the same row before it removes', async () => {
		const servers = client([{ ...home }]);
		const controller = new ServersController(servers);
		await controller.servers.load();
		assert.equal(await controller.remove(home), false);
		assert.equal(controller.confirmingRemoveId, 'home');
		assert.deepEqual(servers.calls, []);
		assert.equal(await controller.remove(home), true);
		assert.deepEqual(servers.calls, ['remove home']);
		assert.deepEqual(controller.servers.value, []);
	});

	test('editing another server cancels a pending remove', async () => {
		const controller = new ServersController(client([{ ...home }]));
		await controller.remove(home);
		controller.edit({ kind: 'update', server: home });
		assert.equal(controller.confirmingRemoveId, null);
		assert.deepEqual(controller.draft, { name: 'Home', url: 'https://home.test/', code: '' });
	});
});
