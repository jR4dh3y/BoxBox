import { apiRequest } from '#lib/api/client.js';

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

interface DesktopActionResponse {
	ok: true;
}

export async function listDesktopApps(
	path: string,
	signal?: AbortSignal
): Promise<DesktopAppsResponse> {
	const value = await apiRequest<unknown>('/desktop/apps', { params: { path }, signal });
	if (
		typeof value !== 'object' ||
		value === null ||
		!('mimeType' in value) ||
		typeof value.mimeType !== 'string' ||
		!('apps' in value) ||
		!Array.isArray(value.apps)
	)
		throw new Error('Invalid application list');
	const apps: DesktopApplication[] = [];
	for (const item of value.apps) {
		if (
			typeof item !== 'object' ||
			item === null ||
			!('id' in item) ||
			typeof item.id !== 'string' ||
			!('name' in item) ||
			typeof item.name !== 'string' ||
			!('isDefault' in item) ||
			typeof item.isDefault !== 'boolean'
		)
			throw new Error('Invalid application entry');
		const application: DesktopApplication = {
			id: item.id,
			name: item.name,
			isDefault: item.isDefault
		};
		if ('icon' in item && typeof item.icon === 'string') application.icon = item.icon;
		apps.push(application);
	}
	return { mimeType: value.mimeType, apps };
}

export async function openDesktopWithApp(path: string, appId: string): Promise<void> {
	await apiRequest<DesktopActionResponse>('/desktop/open-with', {
		method: 'POST',
		body: { path, appId }
	});
}

export async function openDesktopTerminal(path: string): Promise<void> {
	await apiRequest<DesktopActionResponse>('/desktop/terminal', {
		method: 'POST',
		body: { path }
	});
}
