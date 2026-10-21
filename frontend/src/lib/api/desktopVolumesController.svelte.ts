import {
	listDesktopVolumes,
	runDesktopVolumeAction,
	type DesktopVolume,
	type DesktopVolumeAction,
	type DesktopVolumesResponse
} from './desktopVolumes';

export type DesktopVolumesLoadState =
	| { kind: 'loading' }
	| { kind: 'ready'; response: DesktopVolumesResponse }
	| { kind: 'failed'; message: string };

export type DesktopVolumeActionState =
	| { kind: 'idle' }
	| { kind: 'running'; volumeId: string; action: DesktopVolumeAction }
	| { kind: 'failed'; volumeId: string; action: DesktopVolumeAction; message: string };

export type DesktopVolumesNotice = { kind: 'success' | 'error'; message: string } | null;

export interface DesktopVolumesClient {
	list(signal?: AbortSignal): Promise<DesktopVolumesResponse>;
	act(volumeId: string, action: DesktopVolumeAction): Promise<DesktopVolumesResponse>;
}

const defaultClient: DesktopVolumesClient = {
	list: listDesktopVolumes,
	act: runDesktopVolumeAction
};

function errorMessage(error: unknown): string {
	if (error instanceof Error && error.message.trim() !== '') return error.message;
	return 'The removable drive operation could not be completed';
}

function isAbortError(error: unknown): boolean {
	return error instanceof Error && error.name === 'AbortError';
}

function successMessage(volume: DesktopVolume, action: DesktopVolumeAction): string {
	switch (action) {
		case 'mount':
			return `Mounted ${volume.name}.`;
		case 'unmount':
			return `Unmounted ${volume.name}. It is safe to remove.`;
		case 'eject':
			return `Ejected ${volume.name}.`;
	}
}

/**
 * Owns the removable-drive panel's request state so the component only renders.
 * Every successful action replaces the list with the backend's fresh snapshot.
 */
export class DesktopVolumesController {
	loadState = $state<DesktopVolumesLoadState>({ kind: 'loading' });
	actionState = $state<DesktopVolumeActionState>({ kind: 'idle' });
	notice = $state<DesktopVolumesNotice>(null);
	refreshing = $state(false);

	#client: DesktopVolumesClient;
	#revision = 0;

	constructor(client: DesktopVolumesClient = defaultClient) {
		this.#client = client;
	}

	async load(signal?: AbortSignal): Promise<void> {
		const revision = ++this.#revision;
		this.notice = null;
		if (this.loadState.kind !== 'ready') {
			this.loadState = { kind: 'loading' };
		}
		this.refreshing = true;
		try {
			const response = await this.#client.list(signal);
			if (revision !== this.#revision) return;
			this.loadState = { kind: 'ready', response };
		} catch (error) {
			if (revision !== this.#revision || isAbortError(error)) return;
			const message = errorMessage(error);
			if (this.loadState.kind === 'ready') {
				this.notice = { kind: 'error', message };
			} else {
				this.loadState = { kind: 'failed', message };
			}
		} finally {
			if (revision === this.#revision) {
				this.refreshing = false;
			}
		}
	}

	async act(volume: DesktopVolume, action: DesktopVolumeAction): Promise<boolean> {
		if (this.actionState.kind === 'running') return false;
		this.actionState = { kind: 'running', volumeId: volume.id, action };
		this.notice = null;
		try {
			const response = await this.#client.act(volume.id, action);
			this.loadState = { kind: 'ready', response };
			this.actionState = { kind: 'idle' };
			this.notice = { kind: 'success', message: successMessage(volume, action) };
			return true;
		} catch (error) {
			const message = errorMessage(error);
			this.actionState = { kind: 'failed', volumeId: volume.id, action, message };
			this.notice = { kind: 'error', message };
			return false;
		}
	}
}
