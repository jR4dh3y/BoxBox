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

type DesktopClipboardRequest =
	| { operation: DesktopClipboardOperation; paths: string[] }
	| { operation: 'clear-cut'; paths: string[] };

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function parseDesktopClipboard(value: unknown): DesktopClipboardContents | null {
	if (value === null) return null;
	if (!isRecord(value)) throw new Error('Invalid desktop clipboard response');
	if (value.operation !== 'copy' && value.operation !== 'cut') {
		throw new Error('Invalid desktop clipboard operation');
	}
	if (!Array.isArray(value.items)) throw new Error('Invalid desktop clipboard items');

	const items: DesktopClipboardItem[] = [];
	for (const item of value.items) {
		if (
			!isRecord(item) ||
			typeof item.path !== 'string' ||
			typeof item.name !== 'string' ||
			typeof item.isDir !== 'boolean'
		) {
			throw new Error('Invalid desktop clipboard item');
		}
		items.push({ path: item.path, name: item.name, isDir: item.isDir });
	}

	return { operation: value.operation, items };
}

async function sendDesktopClipboardRequest(request: DesktopClipboardRequest): Promise<void> {
	const response = await fetch('/api/v1/desktop/clipboard', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(request)
	});
	if (!response.ok) throw new Error('Could not update the desktop file clipboard');
}

export async function getDesktopClipboard(): Promise<DesktopClipboardContents | null> {
	const response = await fetch('/api/v1/desktop/clipboard');
	if (!response.ok) throw new Error('Could not read the desktop file clipboard');
	const value: unknown = await response.json();
	return parseDesktopClipboard(value);
}

export function setDesktopClipboard(
	paths: readonly string[],
	operation: DesktopClipboardOperation
): Promise<void> {
	if (paths.length === 0) throw new Error('Select at least one file');
	return sendDesktopClipboardRequest({ operation, paths: [...paths] });
}

/** Clear only the unchanged cut selection after its move job succeeds. */
export function clearDesktopCutClipboard(paths: readonly string[]): Promise<void> {
	if (paths.length === 0) throw new Error('Select at least one file');
	return sendDesktopClipboardRequest({ operation: 'clear-cut', paths: [...paths] });
}
