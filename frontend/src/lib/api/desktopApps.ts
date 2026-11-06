import { isRemotePath } from './remotePaths';
import { api, buildUrl } from './client';
import { array, boolean, ok, optionalString, record, string } from './parse';

export interface DesktopApplication {
	id: string;
	name: string;
	icon?: string;
	isDefault: boolean;
}

/** Recommended apps are associated with the file type; other apps accept any file. */
export interface DesktopAppsResponse {
	mimeType: string;
	recommended: DesktopApplication[];
	other: DesktopApplication[];
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
	if (isRemotePath(path)) throw new Error('Native applications require a file on this computer');
	const value = record(
		await api.get<unknown>('/desktop/apps', { path }, signal),
		'application list'
	);
	return {
		mimeType: string(value.mimeType, 'MIME type'),
		recommended: array(value.recommended, 'recommended applications').map(parseApplication),
		other: array(value.other, 'other applications').map(parseApplication)
	};
}

export async function openDesktopWithApp(path: string, appId: string): Promise<void> {
	if (isRemotePath(path)) throw new Error('Native applications require a file on this computer');
	ok(await api.post<unknown>('/desktop/open-with', { path, appId }), 'open-with response');
}

/** The private desktop API resolves installed entry icons; host paths never become image URLs. */
export function desktopAppIconUrl(appId: string): string {
	return buildUrl('/desktop/apps/icon', { appId });
}

export async function setDesktopDefaultApp(path: string, appId: string): Promise<void> {
	if (isRemotePath(path)) throw new Error('Native applications require a file on this computer');
	ok(
		await api.post<unknown>('/desktop/apps/default', { path, appId }),
		'default application response'
	);
}

export async function openDesktopTerminal(path: string): Promise<void> {
	if (isRemotePath(path)) throw new Error('Native applications require a file on this computer');
	ok(await api.post<unknown>('/desktop/terminal', { path }), 'terminal response');
}
