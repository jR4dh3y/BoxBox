import type { FileInfo } from '#lib/api/files.js';
import type { SortDir, SortField } from '#lib/types/files.js';

// Natural order: "9.txt" before "10.txt". Case and accents only decide otherwise equal names.
const compareNames = new Intl.Collator(undefined, { numeric: true }).compare;

interface SortKey {
	item: FileInfo;
	hidden: boolean;
	/** Size, time, or 0 for folders and 1 for files; names compare separately. */
	value: number;
}

/**
 * Returns a sorted copy. Hidden entries always come last, and folders lead a name or type sort in
 * both directions; otherwise `desc` reverses the whole order, ties included.
 */
export function sortFiles(
	items: readonly FileInfo[],
	sortBy: SortField,
	sortDir: SortDir
): FileInfo[] {
	const direction = sortDir === 'desc' ? -1 : 1;
	const byValue = sortBy === 'size' || sortBy === 'modTime';
	const keys: SortKey[] = items.map((item) => ({
		item,
		hidden: item.name.startsWith('.'),
		value:
			sortBy === 'size'
				? item.size
				: sortBy === 'modTime'
					? Date.parse(item.modTime) || 0
					: Number(!item.isDir)
	}));
	keys.sort((left, right) => {
		if (left.hidden !== right.hidden) return left.hidden ? 1 : -1;
		if ((sortBy === 'type' || sortBy === 'name') && left.value !== right.value) {
			return left.value - right.value;
		}
		const comparison =
			(byValue && left.value - right.value) || compareNames(left.item.name, right.item.name);
		return direction * Math.sign(comparison);
	});
	return keys.map((key) => key.item);
}

/** Name-ordered entries, without hidden ones unless `includeHidden`, for pickers and suggestions. */
export function visibleByName(items: readonly FileInfo[], includeHidden: boolean): FileInfo[] {
	const visible = includeHidden ? items : items.filter((item) => !item.name.startsWith('.'));
	return sortFiles(visible, 'name', 'asc');
}
