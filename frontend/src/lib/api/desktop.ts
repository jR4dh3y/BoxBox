import { api } from './client';
import { ok } from './parse';
import { isRemotePath } from './remotePaths';

export function isLocalDesktop(): boolean {
	return (
		typeof document !== 'undefined' &&
		document.querySelector('meta[name="boxbox-desktop"]') !== null
	);
}

/** Native capabilities belong to the location, even inside a desktop window. */
export function isNativePath(path: string): boolean {
	return isLocalDesktop() && !isRemotePath(path);
}

/** Open a file or folder in the OS default application. */
export async function openDesktopFile(path: string): Promise<void> {
	if (!isNativePath(path)) throw new Error('Native apps can only open files on this computer');
	ok(await api.post<unknown>('/desktop/open', { path }), 'open response');
}
