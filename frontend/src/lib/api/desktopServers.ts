import { parseDeploymentIcon, type DeploymentIcon } from '#lib/desktop/deploymentIcon.js';
import { api } from './client';
import { array, nonEmptyString, record, string } from './parse';
import { invalidateRemoteServers } from './remotePaths';

export interface DesktopServer {
	id: string;
	name: string;
	url: string;
	expiresAt: string;
	icon?: DeploymentIcon;
}

export type DesktopServerSave =
	| { kind: 'create'; name: string; url: string; code: string; icon?: DeploymentIcon }
	| { kind: 'update'; id: string; name: string; icon?: DeploymentIcon };

function parseServer(value: unknown): DesktopServer {
	const server = record(value, 'server');
	return {
		icon: parseDeploymentIcon(server.icon),
		id: nonEmptyString(server.id, 'server id'),
		name: string(server.name, 'server name'),
		url: nonEmptyString(server.url, 'server URL'),
		expiresAt: nonEmptyString(server.expiresAt, 'pairing expiry')
	};
}

export async function listDesktopServers(): Promise<DesktopServer[]> {
	const result = record(await api.get<unknown>('/desktop/servers'), 'server list');
	return array(result.servers, 'servers').map(parseServer);
}

export async function saveDesktopServer(command: DesktopServerSave): Promise<DesktopServer> {
	const result =
		command.kind === 'create'
			? await api.post<unknown>('/desktop/servers', {
					name: command.name,
					icon: command.icon ?? 'server',
					url: command.url,
					code: command.code,
					clientName: 'BoxBox Desktop'
				})
			: await api.patch<unknown>(`/desktop/servers/${encodeURIComponent(command.id)}`, {
					name: command.name,
					icon: command.icon ?? 'server'
				});
	invalidateRemoteServers();
	return parseServer(result);
}

export async function pairDesktopServer(id: string, code: string): Promise<DesktopServer> {
	const result = await api.post<unknown>(`/desktop/servers/${encodeURIComponent(id)}/pair`, {
		code,
		clientName: 'BoxBox Desktop'
	});
	invalidateRemoteServers();
	return parseServer(result);
}

export async function removeDesktopServer(id: string): Promise<void> {
	await api.delete<void>(`/desktop/servers/${encodeURIComponent(id)}`);
	invalidateRemoteServers();
}
