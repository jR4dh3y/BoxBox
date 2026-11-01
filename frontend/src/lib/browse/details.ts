import type { FileInfo } from '../api/files';

/** Folder sizes are unknown until scanned; total only the selected files. */
export function summarizeSelection(files: readonly FileInfo[]) {
	const folders = files.filter((file) => file.isDir).length;
	return {
		count: files.length,
		folders,
		files: files.length - folders,
		bytes: files.reduce((sum, file) => sum + (file.isDir ? 0 : file.size), 0)
	};
}

export function parentFolder(path: string): string {
	return path.slice(0, Math.max(0, path.lastIndexOf('/')));
}

export function fullFileDate(value: string): string {
	const date = new Date(value);
	return Number.isNaN(date.getTime())
		? 'Unknown'
		: new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(date);
}
