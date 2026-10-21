import { api } from './client';

export interface DesktopTrashItem {
	id: string;
	name: string;
	originalPath: string;
	deletedAt: string;
	isDir: boolean;
	canRestore: boolean;
}

export interface DesktopTrashListResponse {
	items: DesktopTrashItem[];
}

export interface DesktopTrashItemResponse {
	item: DesktopTrashItem;
}

export interface DesktopTrashActionResponse {
	ok: boolean;
}

interface TrashPathRequest {
	path: string;
}

interface TrashItemRequest {
	id: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function requiredString(value: unknown, label: string): string {
	if (typeof value !== 'string' || value.length === 0) {
		throw new Error(`Invalid desktop trash ${label}`);
	}
	return value;
}

function parseDesktopTrashItem(value: unknown): DesktopTrashItem {
	if (!isRecord(value)) throw new Error('Invalid desktop trash item');
	const deletedAt = requiredString(value.deletedAt, 'deletion time');
	if (typeof value.originalPath !== 'string')
		throw new Error('Invalid desktop trash original path');
	if (!Number.isFinite(Date.parse(deletedAt)))
		throw new Error('Invalid desktop trash deletion time');
	if (typeof value.isDir !== 'boolean' || typeof value.canRestore !== 'boolean') {
		throw new Error('Invalid desktop trash item');
	}
	return {
		id: requiredString(value.id, 'item ID'),
		name: requiredString(value.name, 'item name'),
		originalPath: value.originalPath,
		deletedAt,
		isDir: value.isDir,
		canRestore: value.canRestore
	};
}

function parseDesktopTrashList(value: unknown): DesktopTrashListResponse {
	if (!isRecord(value) || !Array.isArray(value.items)) {
		throw new Error('Invalid desktop trash list');
	}
	return { items: value.items.map((item: unknown) => parseDesktopTrashItem(item)) };
}

function parseDesktopTrashItemResponse(value: unknown): DesktopTrashItemResponse {
	if (!isRecord(value)) throw new Error('Invalid desktop trash response');
	return { item: parseDesktopTrashItem(value.item) };
}

function parseDesktopTrashAction(value: unknown): DesktopTrashActionResponse {
	if (!isRecord(value) || value.ok !== true) {
		throw new Error('Invalid desktop trash action response');
	}
	return { ok: true };
}

/** List the current user's freedesktop trash entries. */
export async function listDesktopTrash(signal?: AbortSignal): Promise<DesktopTrashListResponse> {
	return parseDesktopTrashList(await api.get<unknown>('/desktop/trash', undefined, signal));
}

/** Move a virtual BoxBox path into the desktop trash. */
export async function trashDesktopFile(path: string): Promise<DesktopTrashItemResponse> {
	const body: TrashPathRequest = { path };
	return parseDesktopTrashItemResponse(await api.post<unknown>('/desktop/trash', body));
}

/** Restore one trash entry to its recorded original path. */
export async function restoreDesktopTrashItem(id: string): Promise<DesktopTrashActionResponse> {
	const body: TrashItemRequest = { id };
	return parseDesktopTrashAction(await api.post<unknown>('/desktop/trash/restore', body));
}

/** Permanently remove one trash entry and its contents. */
export async function deleteDesktopTrashItem(id: string): Promise<DesktopTrashActionResponse> {
	const body: TrashItemRequest = { id };
	return parseDesktopTrashAction(await api.post<unknown>('/desktop/trash/delete', body));
}
