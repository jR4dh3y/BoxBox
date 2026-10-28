import type { DeploymentIcon } from './deploymentIcon';
import { SvelteMap } from 'svelte/reactivity';
import { api, ApiRequestError } from '#lib/api/client.js';
import { array, record, nonEmptyString } from '#lib/api/parse.js';
import { qualifyRemotePath } from '#lib/api/remotePaths.js';
import {
	listDesktopServers,
	pairDesktopServer,
	removeDesktopServer,
	saveDesktopServer,
	type DesktopServer,
	type DesktopServerSave
} from '#lib/api/desktopServers.js';
import { PendingAction, RemoteValue } from './remote.svelte';

export type ServerAction = 'save' | 'pair' | 'remove';
export type ServerEditor =
	| { kind: 'closed' }
	| { kind: 'create' }
	| { kind: 'update'; server: DesktopServer }
	| { kind: 'pair'; server: DesktopServer };

export interface ServersClient {
	list(): Promise<DesktopServer[]>;
	save(command: DesktopServerSave): Promise<DesktopServer>;
	pair(id: string, code: string): Promise<DesktopServer>;
	remove(id: string): Promise<void>;
}
export type ServerStatus =
	| { kind: 'checking' }
	| { kind: 'ready'; paths: { name: string; path: string }[] }
	| { kind: 'failed'; message: string; needsPairing: boolean };

export async function probeDesktopServer(id: string): Promise<{ name: string; path: string }[]> {
	const value = await api.get<unknown>(`/desktop/servers/${encodeURIComponent(id)}/api/v1/files`);
	return array(record(value, 'roots response').roots, 'roots').map((value) => {
		const name = nonEmptyString(record(value, 'root').name, 'root name');
		return { name, path: qualifyRemotePath(id, name) };
	});
}

const defaultClient: ServersClient = {
	list: listDesktopServers,
	save: saveDesktopServer,
	pair: pairDesktopServer,
	remove: removeDesktopServer
};

export class ServersController {
	#checks = new SvelteMap<string, number>();
	readonly statuses = new SvelteMap<string, ServerStatus>();
	readonly servers: RemoteValue<DesktopServer[]>;
	readonly action = new PendingAction<ServerAction>('The server list could not be changed');
	editor = $state<ServerEditor>({ kind: 'closed' });
	draft = $state<{ name: string; url: string; code: string; icon: DeploymentIcon }>({
		name: '',
		url: '',
		code: '',
		icon: 'server'
	});
	confirmingRemoveId = $state<string | null>(null);
	#client: ServersClient;
	#onChanged: () => void;

	constructor(client: ServersClient = defaultClient, onChanged: () => void = () => {}) {
		this.#client = client;
		this.#onChanged = onChanged;
		this.servers = new RemoteValue(() => client.list(), 'Paired servers could not be loaded');
	}
	edit(editor: ServerEditor): void {
		if (this.action.busy) return;
		this.editor = editor;
		this.draft =
			editor.kind === 'update' || editor.kind === 'pair'
				? {
						name: editor.server.name,
						url: editor.server.url,
						code: '',
						icon: editor.server.icon ?? 'server'
					}
				: { name: '', url: '', code: '', icon: 'server' };
		this.confirmingRemoveId = null;
		this.action.reset();
	}
	async check(server: DesktopServer, { background = false } = {}): Promise<void> {
		const version = (this.#checks.get(server.id) ?? 0) + 1;
		this.#checks.set(server.id, version);
		if (!background || !this.statuses.has(server.id))
			this.statuses.set(server.id, { kind: 'checking' });
		try {
			const paths = await probeDesktopServer(server.id);
			if (this.#checks.get(server.id) === version)
				this.statuses.set(server.id, { kind: 'ready', paths });
		} catch (error) {
			if (this.#checks.get(server.id) !== version) return;
			this.statuses.set(server.id, {
				kind: 'failed',
				message: error instanceof Error ? error.message : 'Server is unavailable',
				needsPairing: error instanceof ApiRequestError && error.code === 'REMOTE_AUTH_REQUIRED'
			});
		}
	}
	async refresh(): Promise<void> {
		await this.servers.load();
		await Promise.all(
			(this.servers.value ?? []).map((server) => this.check(server, { background: true }))
		);
	}
	async save(): Promise<boolean> {
		const editor = this.editor;
		if (editor.kind === 'closed') return false;
		const { name, url, code, icon } = this.draft;
		this.draft.code = '';
		const done = await this.action.run(
			'editor',
			editor.kind === 'pair' ? 'pair' : 'save',
			async () => {
				if (editor.kind === 'pair') await this.#client.pair(editor.server.id, code);
				else
					await this.#client.save(
						editor.kind === 'update'
							? { kind: 'update', id: editor.server.id, name, icon }
							: { kind: 'create', name, url, code, icon }
					);
			}
		);
		if (!done) return false;
		this.editor = { kind: 'closed' };
		this.#onChanged();
		await this.refresh();
		return true;
	}
	async remove(server: DesktopServer): Promise<boolean> {
		if (this.confirmingRemoveId !== server.id) {
			this.confirmingRemoveId = server.id;
			this.action.reset();
			return false;
		}
		const done = await this.action.run(server.id, 'remove', () => this.#client.remove(server.id));
		if (!done) return false;
		this.confirmingRemoveId = null;
		if (
			(this.editor.kind === 'update' || this.editor.kind === 'pair') &&
			this.editor.server.id === server.id
		)
			this.edit({ kind: 'closed' });
		this.#onChanged();
		await this.refresh();
		return true;
	}
}
