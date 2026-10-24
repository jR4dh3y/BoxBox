import type { FileInfo, FileList } from './files';
import { array, isRecord, nonEmptyString, record } from './parse';

type DirectoryEntry = Omit<FileInfo, 'path'>;

function isDirectoryEntry(value: unknown): value is DirectoryEntry {
	return (
		isRecord(value) &&
		typeof value.name === 'string' &&
		value.name.length > 0 &&
		value.name !== '.' &&
		value.name !== '..' &&
		!value.name.includes('/') &&
		!value.name.includes('\0') &&
		typeof value.size === 'number' &&
		Number.isFinite(value.size) &&
		typeof value.isDir === 'boolean' &&
		typeof value.modTime === 'string' &&
		typeof value.permissions === 'string' &&
		(value.mimeType === undefined || typeof value.mimeType === 'string')
	);
}

/** Expand shared parent paths once, at the boundary used by normal and early fetches. */
export function parseDirectory(value: unknown): FileList {
	const response = record(value, 'directory');
	const path = nonEmptyString(response.path, 'directory path');
	const items = array(response.items, 'directory entries').map((item): FileInfo => {
		if (!isDirectoryEntry(item)) throw new Error('Invalid directory entry');
		return {
			name: item.name,
			path: `${path}/${item.name}`,
			size: item.size,
			isDir: item.isDir,
			modTime: item.modTime,
			permissions: item.permissions,
			mimeType: item.mimeType
		};
	});
	return { path, items };
}
