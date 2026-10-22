import { api } from './client';
import { ok } from './parse';

export function isLocalDesktop(): boolean {
	return (
		typeof document !== 'undefined' &&
		document.querySelector('meta[name="boxbox-desktop"]') !== null
	);
}

/** Open a file or folder in the OS default application. */
export async function openDesktopFile(path: string): Promise<void> {
	ok(await api.post<unknown>('/desktop/open', { path }), 'open response');
}

export function openConnectionManager(): void {
	if ('__electrobunSendToHost' in window && typeof window.__electrobunSendToHost === 'function') {
		window.__electrobunSendToHost({ type: 'boxbox:connections' });
	}
}
