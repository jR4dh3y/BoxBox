import type { DriveStats, MountPoint } from '#lib/api/files.js';
import type { RemoteServer } from '#lib/api/remotePaths.js';
export type RemoteCatalogState =
	{ kind: 'loading' } | { kind: 'ready' } | { kind: 'error'; message: string };

export interface RemoteCatalogSnapshot {
	status: RemoteCatalogState;
	states: Record<string, RemoteCatalogState>;
	roots: MountPoint[];
	drives: DriveStats[];
	servers: RemoteServer[];
}
