import {
	deleteDesktopTrashItem,
	listDesktopTrash,
	restoreDesktopTrashItem,
	type DesktopTrashItem
} from './desktopTrash';

export type DesktopTrashLoadState =
	| { kind: 'loading' }
	| { kind: 'ready'; items: DesktopTrashItem[] }
	| { kind: 'failed'; message: string };

export type DesktopTrashActionState =
	| { kind: 'idle' }
	| { kind: 'running'; itemId: string; action: 'restore' | 'delete' }
	| { kind: 'failed'; itemId: string; message: string };

export interface DesktopTrashClient {
	list(signal?: AbortSignal): Promise<DesktopTrashItem[]>;
	restore(id: string): Promise<void>;
	delete(id: string): Promise<void>;
}

const defaultClient: DesktopTrashClient = {
	list: listDesktopTrash,
	restore: restoreDesktopTrashItem,
	delete: deleteDesktopTrashItem
};

function errorMessage(error: unknown): string {
	if (error instanceof Error && error.message.trim() !== '') return error.message;
	return 'The Trash operation could not be completed';
}

function isAbortError(error: unknown): boolean {
	return error instanceof Error && error.name === 'AbortError';
}

/** Owns Trash loading, selection, and mutation state outside the view. */
export class DesktopTrashController {
	loadState = $state<DesktopTrashLoadState>({ kind: 'loading' });
	actionState = $state<DesktopTrashActionState>({ kind: 'idle' });
	selectedId = $state<string | null>(null);
	refreshing = $state(false);
	refreshError = $state<string | null>(null);

	#client: DesktopTrashClient;
	#loadRevision = 0;

	constructor(client: DesktopTrashClient = defaultClient) {
		this.#client = client;
	}

	async load(signal?: AbortSignal): Promise<void> {
		const revision = ++this.#loadRevision;
		this.refreshError = null;
		if (this.loadState.kind !== 'ready') this.loadState = { kind: 'loading' };
		this.refreshing = true;
		try {
			const items = await this.#client.list(signal);
			if (revision !== this.#loadRevision) return;
			this.loadState = { kind: 'ready', items };
			if (this.selectedId && !items.some((item) => item.id === this.selectedId)) {
				this.selectedId = null;
			}
		} catch (error) {
			if (revision !== this.#loadRevision || isAbortError(error)) return;
			const message = errorMessage(error);
			if (this.loadState.kind === 'ready') {
				this.refreshError = message;
			} else {
				this.loadState = { kind: 'failed', message };
			}
		} finally {
			if (revision === this.#loadRevision) this.refreshing = false;
		}
	}

	async restore(id: string): Promise<boolean> {
		return this.#runAction(id, 'restore', (itemId) => this.#client.restore(itemId));
	}

	async delete(id: string): Promise<boolean> {
		return this.#runAction(id, 'delete', (itemId) => this.#client.delete(itemId));
	}

	async #runAction(
		id: string,
		action: 'restore' | 'delete',
		operation: (itemId: string) => Promise<void>
	): Promise<boolean> {
		if (this.actionState.kind === 'running') return false;
		this.actionState = { kind: 'running', itemId: id, action };
		try {
			await operation(id);
			this.actionState = { kind: 'idle' };
			await this.load();
			return true;
		} catch (error) {
			this.actionState = { kind: 'failed', itemId: id, message: errorMessage(error) };
			return false;
		}
	}
}
