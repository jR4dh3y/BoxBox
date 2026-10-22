import { api } from './client';
import { array, boolean, ok, record, string } from './parse';

export type DesktopClipboardOperation = 'copy' | 'cut';

export interface DesktopClipboardItem {
	path: string;
	name: string;
	isDir: boolean;
}

export interface DesktopClipboardContents {
	operation: DesktopClipboardOperation;
	items: DesktopClipboardItem[];
}

function parseItem(value: unknown): DesktopClipboardItem {
	const item = record(value, 'desktop clipboard item');
	return {
		path: string(item.path, 'desktop clipboard item'),
		name: string(item.name, 'desktop clipboard item'),
		isDir: boolean(item.isDir, 'desktop clipboard item')
	};
}

function parseDesktopClipboard(value: unknown): DesktopClipboardContents | null {
	if (value === null) return null;
	const contents = record(value, 'desktop clipboard response');
	if (contents.operation !== 'copy' && contents.operation !== 'cut') {
		throw new Error('Invalid desktop clipboard operation');
	}
	return {
		operation: contents.operation,
		items: array(contents.items, 'desktop clipboard items').map(parseItem)
	};
}

async function post(operation: DesktopClipboardOperation | 'clear-cut', paths: readonly string[]) {
	if (paths.length === 0) throw new Error('Select at least one file');
	ok(
		await api.post<unknown>('/desktop/clipboard', { operation, paths: [...paths] }),
		'desktop clipboard response'
	);
}

export async function getDesktopClipboard(): Promise<DesktopClipboardContents | null> {
	return parseDesktopClipboard(await api.get<unknown>('/desktop/clipboard'));
}

export function setDesktopClipboard(
	paths: readonly string[],
	operation: DesktopClipboardOperation
): Promise<void> {
	return post(operation, paths);
}

/** Clear only the unchanged cut selection after its move job succeeds. */
export function clearDesktopCutClipboard(paths: readonly string[]): Promise<void> {
	return post('clear-cut', paths);
}
