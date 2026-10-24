import {
	deleteDesktopTrashItem,
	listDesktopTrash,
	restoreDesktopTrashItem,
	type DesktopTrashItem
} from '#lib/api/desktopTrash.js';
import { PendingAction, RemoteValue } from './remote.svelte';

export type TrashAction = 'restore' | 'delete';

export interface TrashClient {
	list(signal?: AbortSignal): Promise<DesktopTrashItem[]>;
	restore(id: string): Promise<void>;
	delete(id: string): Promise<void>;
}

const defaultClient: TrashClient = {
	list: listDesktopTrash,
	restore: restoreDesktopTrashItem,
	delete: deleteDesktopTrashItem
};

export class TrashController {
	readonly items: RemoteValue<DesktopTrashItem[]>;
	readonly action = new PendingAction<TrashAction>('The Trash operation could not be completed');
	selectedId = $state<string | null>(null);
	/** Permanent deletion takes a second click on the same button. */
	confirmingDelete = $state(false);

	#client: TrashClient;

	constructor(client: TrashClient = defaultClient) {
		this.#client = client;
		this.items = new RemoteValue((signal) => client.list(signal), 'Trash could not be loaded');
	}

	get selected(): DesktopTrashItem | null {
		return this.items.value?.find((item) => item.id === this.selectedId) ?? null;
	}

	select(id: string): void {
		if (this.action.busy) return;
		this.selectedId = id;
		this.confirmingDelete = false;
		this.action.reset();
	}

	/** Restores or deletes the selected item, then reloads the list. */
	async run(action: TrashAction): Promise<boolean> {
		const item = this.selected;
		if (!item || (action === 'restore' && !item.canRestore)) return false;
		if (action === 'delete' && !this.confirmingDelete) {
			this.confirmingDelete = true;
			return false;
		}
		const done = await this.action.run(item.id, action, () => this.#client[action](item.id));
		if (!done) return false;
		this.confirmingDelete = false;
		await this.items.load();
		return true;
	}
}
