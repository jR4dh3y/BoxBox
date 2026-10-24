import { isLocalDesktop } from './desktop';
import { api } from './client';
import { array, nonEmptyString, record, string } from './parse';

export type PathReference =
	{ kind: 'local'; path: string } | { kind: 'remote'; serverId: string; path: string };

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function validateRemotePath(path: string): void {
	if (
		!path ||
		path.startsWith('/') ||
		path.includes('\\') ||
		Array.from(path).some(
			(character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127
		) ||
		path.split('/').some((part) => !part || part === '.' || part === '..') ||
		path.startsWith('@remote/')
	) {
		throw new Error('Invalid remote path');
	}
}

export function parseRemotePath(reference: string): PathReference {
	if (reference !== '@remote' && !reference.startsWith('@remote/'))
		return { kind: 'local', path: reference };
	const [, serverId, ...parts] = reference.split('/');
	if (!serverId || !UUID.test(serverId)) throw new Error('Invalid remote server ID');
	const path = parts.join('/');
	validateRemotePath(path);
	return { kind: 'remote', serverId, path };
}

export function isRemotePath(path: string): boolean {
	return parseRemotePath(path).kind === 'remote';
}

export function qualifyRemotePath(serverId: string, path: string): string {
	if (!UUID.test(serverId)) throw new Error('Invalid remote server ID');
	validateRemotePath(path);
	return `@remote/${serverId}/${path}`;
}

export function qualifyPath(source: PathReference, path: string): string {
	return source.kind === 'remote' ? qualifyRemotePath(source.serverId, path) : path;
}

export function sameSource(a: PathReference, b: PathReference): boolean {
	return a.kind === 'local' ? b.kind === 'local' : b.kind === 'remote' && a.serverId === b.serverId;
}

export function sourceEndpoint(source: PathReference, endpoint: string): string {
	return source.kind === 'remote'
		? `/desktop/servers/${source.serverId}/api/v1${endpoint}`
		: endpoint;
}

export function encodeRoutePath(path: string): string {
	return path.split('/').map(encodeURIComponent).join('/');
}

export function pathEndpoint(reference: string, prefix: string): string {
	const source = parseRemotePath(reference);
	return sourceEndpoint(source, `${prefix}/${encodeRoutePath(source.path)}`);
}

export function number(value: unknown, label: string): number {
	if (typeof value !== 'number' || !Number.isFinite(value)) throw new Error(`Invalid ${label}`);
	return value;
}

export interface RemoteServer {
	id: string;
	name: string;
	url: string;
}
let pendingServers: Promise<RemoteServer[]> | undefined;

/** Only concurrent loads are cached, so subsequent reads always confirm server state. */
export function invalidateRemoteServers(): void {
	pendingServers = undefined;
}

export function listRemoteServers(): Promise<RemoteServer[]> {
	if (pendingServers) return pendingServers;
	const operation = api.get<unknown>('/desktop/servers').then((value) =>
		array(record(value, 'servers response').servers, 'servers').map((item) => {
			const server = record(item, 'server');
			const id = nonEmptyString(server.id, 'server ID');
			if (!UUID.test(id)) throw new Error('Invalid remote server ID');
			const url = new URL(nonEmptyString(server.url, 'server URL'));
			if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash)
				throw new Error('Invalid server URL');
			return { id, name: string(server.name, 'server name'), url: url.href };
		})
	);
	pendingServers = operation;
	void operation
		.finally(() => {
			if (pendingServers === operation) pendingServers = undefined;
		})
		.catch(() => {});
	return operation;
}

/** Local-only servers do not expose desktop management. */
export async function availableRemoteServers(): Promise<RemoteServer[]> {
	if (!isLocalDesktop()) return [];
	try {
		return await listRemoteServers();
	} catch {
		return [];
	}
}

export function remoteSource(server: RemoteServer): PathReference {
	return { kind: 'remote', serverId: server.id, path: '' };
}

export async function collectRemote<T>(load: (server: RemoteServer) => Promise<T[]>): Promise<T[]> {
	const servers = await availableRemoteServers();
	const results = await Promise.allSettled(servers.map(load));
	return results.flatMap((result) => (result.status === 'fulfilled' ? result.value : []));
}
