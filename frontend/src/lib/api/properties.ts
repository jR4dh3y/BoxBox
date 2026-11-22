import { apiRequest } from './client';
import { array, boolean, isRecord, optionalString, record, string } from './parse';
import { number, parseRemotePath, qualifyPath, sourceEndpoint } from './remotePaths';

/** Owner, group and dates, where the server's file system keeps them. */
export interface ItemProperties {
	path: string;
	owner: string | null;
	group: string | null;
	accessed: string | null;
	changed: string | null;
}

export interface SelectionProperties {
	items: ItemProperties[];
	files: number;
	folders: number;
	bytes: number;
	/** False when the server stopped walking, so the totals are a lower bound. */
	complete: boolean;
}

function parseItem(value: unknown, qualify: (path: string) => string): ItemProperties {
	const item = record(value, 'item properties');
	return {
		path: qualify(string(item.path, 'item path')),
		owner: optionalString(item.owner, 'owner') ?? null,
		group: optionalString(item.group, 'group') ?? null,
		accessed: optionalString(item.accessed, 'accessed time') ?? null,
		changed: optionalString(item.changed, 'changed time') ?? null
	};
}

/** Properties of items on one source: the first item's server, or this one. */
export async function getProperties(
	paths: readonly string[],
	signal?: AbortSignal
): Promise<SelectionProperties> {
	const sources = paths.map(parseRemotePath);
	const source = sources[0];
	if (!source) throw new Error('Choose an item first');
	const value = await apiRequest<unknown>(sourceEndpoint(source, '/files'), {
		method: 'POST',
		body: { paths: sources.map((item) => item.path) },
		signal
	});
	if (!isRecord(value)) throw new Error('Invalid properties response');
	return {
		items: array(value.items, 'properties items').map((item) =>
			parseItem(item, (path) => qualifyPath(source, path))
		),
		files: number(value.files, 'file count'),
		folders: number(value.folders, 'folder count'),
		bytes: number(value.bytes, 'byte count'),
		complete: boolean(value.complete, 'complete flag')
	};
}
