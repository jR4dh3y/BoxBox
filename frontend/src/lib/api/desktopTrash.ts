import { isRemotePath } from './remotePaths';
import { api } from './client';
import { array, boolean, nonEmptyString, ok, record, string } from './parse';

export interface DesktopTrashItem {
	id: string;
	name: string;
	originalPath: string;
	deletedAt: string;
	isDir: boolean;
	canRestore: boolean;
}

function parseDesktopTrashItem(value: unknown): DesktopTrashItem {
	const item = record(value, 'desktop trash item');
	const deletedAt = nonEmptyString(item.deletedAt, 'desktop trash deletion time');
	if (!Number.isFinite(Date.parse(deletedAt))) {
		throw new Error('Invalid desktop trash deletion time');
	}
	return {
		id: nonEmptyString(item.id, 'desktop trash item ID'),
		name: nonEmptyString(item.name, 'desktop trash item name'),
		originalPath: string(item.originalPath, 'desktop trash original path'),
		deletedAt,
		isDir: boolean(item.isDir, 'desktop trash item'),
		canRestore: boolean(item.canRestore, 'desktop trash item')
	};
}

/** List the current user's freedesktop trash entries. */
export async function listDesktopTrash(signal?: AbortSignal): Promise<DesktopTrashItem[]> {
	const value = record(await api.get<unknown>('/desktop/trash', undefined, signal), 'trash list');
	return array(value.items, 'trash list').map(parseDesktopTrashItem);
}

/** Move a virtual BoxBox path into the desktop trash. */
export async function trashDesktopFile(path: string): Promise<DesktopTrashItem> {
	if (isRemotePath(path)) throw new Error('Remote files must be deleted on their server');
	const value = record(await api.post<unknown>('/desktop/trash', { path }), 'trash response');
	return parseDesktopTrashItem(value.item);
}

/** Restore one trash entry to its recorded original path. */
export async function restoreDesktopTrashItem(id: string): Promise<void> {
	ok(await api.post<unknown>('/desktop/trash/restore', { id }), 'trash restore response');
}

/** Permanently remove one trash entry and its contents. */
export async function deleteDesktopTrashItem(id: string): Promise<void> {
	ok(await api.post<unknown>('/desktop/trash/delete', { id }), 'trash delete response');
}
