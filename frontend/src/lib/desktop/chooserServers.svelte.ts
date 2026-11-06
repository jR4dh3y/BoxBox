import { listDesktopServers, type DesktopServer } from '#lib/api/desktopServers.js';
import { listRoots, type MountPoint } from '#lib/api/files.js';
import { RemoteValue } from './remote.svelte.js';

export class ChooserServers {
	readonly catalog: RemoteValue<{ server: DesktopServer; roots: RemoteValue<MountPoint[]> }[]>;
	constructor(list = listDesktopServers, roots: typeof listRoots = listRoots) {
		this.catalog = new RemoteValue(async (signal) => {
			const servers = await list(signal);
			return servers.map((server) => ({
				server,
				roots: new RemoteValue(
					async (requestSignal = signal) => (await roots(server, requestSignal)).roots,
					'Server locations could not be loaded'
				)
			}));
		}, 'Saved servers could not be loaded');
	}
	async load(signal: AbortSignal) {
		await this.catalog.load(signal);
		if (signal.aborted) return;
		// Each server publishes independently; an offline server cannot hold up another.
		await Promise.all(this.catalog.value?.map(({ roots }) => roots.load(signal)) ?? []);
	}
}
