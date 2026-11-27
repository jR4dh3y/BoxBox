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

/** What the Details panel shows: the folder, one item, or a count of several. */
export type DetailsContent =
	| { kind: 'folder'; count: number }
	| { kind: 'single'; item: FileInfo }
	| {
			kind: 'many';
			files: number;
			folders: number;
			/** Folders are not walked here; Properties counts what is inside them. */
			bytes: number;
			/** The first names, and how many more there are. */
			names: string[];
			more: number;
	  };

/** The names the panel lists before "and N more". */
const LISTED = 50;

export function detailsContent(
	selected: readonly FileInfo[],
	folderItemCount: number
): DetailsContent {
	const [only] = selected;
	if (selected.length === 0 || !only) return { kind: 'folder', count: folderItemCount };
	if (selected.length === 1) return { kind: 'single', item: only };
	const summary = summarizeSelection(selected);
	return {
		kind: 'many',
		files: summary.files,
		folders: summary.folders,
		bytes: summary.bytes,
		names: selected.slice(0, LISTED).map((item) => item.name),
		more: Math.max(0, selected.length - LISTED)
	};
}
