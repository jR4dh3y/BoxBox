import {
	listDesktopVolumes,
	runDesktopVolumeAction,
	type DesktopVolume,
	type DesktopVolumeAction,
	type DesktopVolumesResponse
} from '#lib/api/desktopVolumes.js';
import { PendingAction, RemoteValue } from './remote.svelte';

export interface VolumesClient {
	list(signal?: AbortSignal): Promise<DesktopVolumesResponse>;
	act(volumeId: string, action: DesktopVolumeAction): Promise<DesktopVolumesResponse>;
}

const defaultClient: VolumesClient = {
	list: listDesktopVolumes,
	act: runDesktopVolumeAction
};

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

/** Every successful action replaces the list with the backend's fresh snapshot. */
export class VolumesController {
	readonly snapshot: RemoteValue<DesktopVolumesResponse>;
	readonly action = new PendingAction<DesktopVolumeAction>(
		'The removable drive operation could not be completed'
	);
	success = $state<string | null>(null);

	#client: VolumesClient;

	constructor(client: VolumesClient = defaultClient) {
		this.#client = client;
		this.snapshot = new RemoteValue(
			(signal) => client.list(signal),
			'Removable drives could not be loaded'
		);
	}

	get busy(): boolean {
		return this.snapshot.refreshing || this.action.busy;
	}

	/** The newest failure, from an action or a refresh. */
	get error(): string | null {
		return this.action.error ?? this.snapshot.refreshError;
	}

	async load(signal?: AbortSignal): Promise<void> {
		this.success = null;
		this.action.reset();
		await this.snapshot.load(signal);
	}

	async act(volume: DesktopVolume, action: DesktopVolumeAction): Promise<boolean> {
		this.success = null;
		const done = await this.action.run(volume.id, action, async () => {
			this.snapshot.set(await this.#client.act(volume.id, action));
		});
		if (done) this.success = successMessage(volume, action);
		return done;
	}
}
