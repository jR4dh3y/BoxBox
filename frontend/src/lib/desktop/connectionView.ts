import type { DesktopServer } from '#lib/api/desktopServers.js';
import type { ServerStatus } from './servers.svelte';

/** What one saved server row shows, and so which single repair action applies. */
export type ConnectionView =
	| { kind: 'checking' }
	| { kind: 'connected'; drives: number; expires: string }
	| { kind: 'needs-pairing'; summary: string }
	| { kind: 'offline'; detail: string };

function formatDate(value: string): string {
	return new Date(value).toLocaleDateString(undefined, {
		day: 'numeric',
		month: 'short',
		year: 'numeric'
	});
}

export function connectionView(
	server: DesktopServer,
	status: ServerStatus | undefined,
	now: number = Date.now()
): ConnectionView {
	// An expired pairing cannot recover by retrying, whatever the last probe said.
	if (!(Date.parse(server.expiresAt) > now))
		return { kind: 'needs-pairing', summary: 'Access expired' };
	switch (status?.kind) {
		case undefined:
		case 'checking':
			return { kind: 'checking' };
		case 'ready':
			return {
				kind: 'connected',
				drives: status.paths.length,
				expires: formatDate(server.expiresAt)
			};
		case 'failed':
			return status.needsPairing
				? { kind: 'needs-pairing', summary: 'Access was revoked' }
				: { kind: 'offline', detail: status.message };
	}
}

export function connectionSummary(view: ConnectionView): string {
	switch (view.kind) {
		case 'checking':
			return 'Checking…';
		case 'connected':
			return `${view.drives} drive${view.drives === 1 ? '' : 's'} · access until ${view.expires}`;
		case 'needs-pairing':
			return view.summary;
		case 'offline':
			return 'Can’t reach this server';
	}
}

/** The address without the scheme, which is always HTTPS for a paired server. */
export function serverAddress(url: string): string {
	try {
		const parsed = new URL(url);
		const path = parsed.pathname === '/' ? '' : parsed.pathname.replace(/\/$/, '');
		return parsed.host + path;
	} catch {
		return url;
	}
}
