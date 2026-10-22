import { api } from './client';
import { array, boolean, ok, optionalString, record, string } from './parse';

export interface DesktopApplication {
	id: string;
	name: string;
	icon?: string;
	isDefault: boolean;
}

export interface DesktopAppsResponse {
	mimeType: string;
	apps: DesktopApplication[];
}

function parseApplication(value: unknown): DesktopApplication {
	const app = record(value, 'application entry');
	return {
		id: string(app.id, 'application id'),
		name: string(app.name, 'application name'),
		icon: optionalString(app.icon, 'application icon'),
		isDefault: boolean(app.isDefault, 'default application flag')
	};
}

export async function listDesktopApps(
	path: string,
	signal?: AbortSignal
): Promise<DesktopAppsResponse> {
	const value = record(
		await api.get<unknown>('/desktop/apps', { path }, signal),
		'application list'
	);
	return {
		mimeType: string(value.mimeType, 'MIME type'),
		apps: array(value.apps, 'application list').map(parseApplication)
	};
}

export async function openDesktopWithApp(path: string, appId: string): Promise<void> {
	ok(await api.post<unknown>('/desktop/open-with', { path, appId }), 'open-with response');
}

export async function openDesktopTerminal(path: string): Promise<void> {
	ok(await api.post<unknown>('/desktop/terminal', { path }), 'terminal response');
}
