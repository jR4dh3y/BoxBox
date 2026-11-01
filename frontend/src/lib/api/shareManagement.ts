/** Authenticated share creation and management; recipient pages use shares.ts. */
import { api, apiRequest } from './client';
import { array, boolean, optionalString, record, string } from './parse';
import {
	collectRemote,
	listRemoteServers,
	number,
	parseRemotePath,
	pathEndpoint,
	qualifyPath,
	remoteSource,
	sourceEndpoint,
	type PathReference,
	type RemoteServer
} from './remotePaths';
import type {
	SharePermissions,
	ShareRecord,
	CreateShareResponse,
	CreateShareOptions,
	ShareListResponse,
	ShareActionResponse
} from './shares';

function parsePermissions(value: unknown): SharePermissions {
	const p = record(value, 'share permissions');
	return {
		view: boolean(p.view, 'view'),
		download: boolean(p.download, 'download'),
		upload: boolean(p.upload, 'upload'),
		delete: boolean(p.delete, 'delete'),
		manage: boolean(p.manage, 'manage'),
		canReplace: boolean(p.canReplace, 'replace')
	};
}

function parseCreatedShare(
	value: unknown,
	source: PathReference,
	server?: RemoteServer
): CreateShareResponse {
	const share = record(value, 'share');
	const url = string(share.url, 'share URL');
	const token = string(share.token, 'share token');
	return {
		id: qualifyPath(source, string(share.id, 'share ID')),
		token,
		url: server
			? new URL(
					`s/${encodeURIComponent(token)}`,
					server.url.endsWith('/') ? server.url : `${server.url}/`
				).href
			: url,
		fileName: string(share.fileName, 'file name'),
		isFolder: boolean(share.isFolder, 'folder flag'),
		permissions: parsePermissions(share.permissions),
		maxUploadBytes: number(share.maxUploadBytes, 'upload limit'),
		createdAt: string(share.createdAt, 'created time'),
		expiresAt: optionalString(share.expiresAt, 'expiry')
	};
}

function parseShares(value: unknown, source: PathReference, server?: RemoteServer): ShareRecord[] {
	return array(record(value, 'shares response').shares, 'shares').map((value) => ({
		...parseCreatedShare(value, source, server),
		path: qualifyPath(source, string(record(value, 'share').path, 'share path'))
	}));
}

/**
 * Create a share link for a file or folder
 * POST /api/v1/shares
 */
export async function createShare(
	path: string,
	options: CreateShareOptions
): Promise<CreateShareResponse> {
	const source = parseRemotePath(path);
	const serverPromise =
		source.kind === 'remote'
			? listRemoteServers().then((servers) => {
					const server = servers.find((server) => server.id === source.serverId);
					if (!server) throw new Error('Remote server is unavailable');
					return server;
				})
			: Promise.resolve(undefined);
	const [server, value] = await Promise.all([
		serverPromise,
		api.post<unknown>(sourceEndpoint(source, '/shares'), {
			path: source.path,
			...(options.permissions ? { permissions: options.permissions } : {}),
			...(options.maxUploadBytes !== undefined ? { maxUploadBytes: options.maxUploadBytes } : {}),
			expiresInSeconds: options.expiresInSeconds ?? null
		})
	]);
	return parseCreatedShare(value, source, server);
}

/**
 * List active share links
 * GET /api/v1/shares
 */
export async function listShares(): Promise<ShareListResponse> {
	const [local, remote] = await Promise.all([
		api.get<unknown>('/shares').then((value) => parseShares(value, { kind: 'local', path: '' })),
		collectRemote(async (server) =>
			parseShares(
				await api.get<unknown>(sourceEndpoint(remoteSource(server), '/shares')),
				remoteSource(server),
				server
			)
		)
	]);
	return { shares: [...local, ...remote] };
}

/**
 * Revoke a share link
 * DELETE /api/v1/shares/{id}
 */
export async function revokeShare(id: string): Promise<ShareActionResponse> {
	const value = await apiRequest<unknown>(pathEndpoint(id, '/shares'), { method: 'DELETE' });
	return value === undefined
		? { success: true }
		: { success: boolean(record(value, 'revoke response').success, 'success') };
}
