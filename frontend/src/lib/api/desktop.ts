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

/**
 * Share links and browser downloads serve files from a server. A path on this computer has
 * neither, so one native item removes them from the whole selection.
 */
export function supportsServerTransfer(items: readonly { path: string }[]): boolean {
	return items.length > 0 && !items.some((item) => isNativePath(item.path));
}

/** Open a file or folder in the OS default application. */
export async function openDesktopFile(path: string): Promise<void> {
	if (!isNativePath(path)) throw new Error('Native apps can only open files on this computer');
	ok(await api.post<unknown>('/desktop/open', { path }), 'open response');
}
