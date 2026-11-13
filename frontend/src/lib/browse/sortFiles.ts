import type { FileInfo } from '#lib/api/files.js';
import type { SortDir, SortField } from '#lib/types/files.js';

// Natural order: "9.txt" before "10.txt". Case and accents only decide otherwise equal names.
const collateNames = new Intl.Collator('en', { numeric: true }).compare;
const compareText = (left: string, right: string) => (left < right ? -1 : left > right ? 1 : 0);
const compareNames = (left: string, right: string) =>
	collateNames(left, right) || compareText(left, right);

export const DEFAULT_SORT_BY = 'modTime';
export const DEFAULT_SORT_DIR = 'desc';

interface SortKey {
	item: FileInfo;
	/** Size or modified time; names compare separately. */
	value: number;
}

/**
 * Folders lead every sort. Time and size ties use ascending natural names, then paths,
 * so recursive search results and repeated reads have a deterministic order.
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
		value: sortBy === 'size' ? item.size : sortBy === 'modTime' ? Date.parse(item.modTime) || 0 : 0
	}));
	keys.sort((left, right) => {
		if (left.item.isDir !== right.item.isDir) return left.item.isDir ? -1 : 1;
		if (byValue && left.value !== right.value)
			return direction * Math.sign(left.value - right.value);
		return (
			(byValue ? 1 : direction) * compareNames(left.item.name, right.item.name) ||
			compareNames(left.item.path, right.item.path)
		);
	});
	return keys.map((key) => key.item);
}

/** Default ordering for pickers and suggestions, filtering hidden entries only when requested. */
export function visibleByDefault(items: readonly FileInfo[], includeHidden: boolean): FileInfo[] {
	const visible = includeHidden ? items : items.filter((item) => !item.name.startsWith('.'));
	return sortFiles(visible, DEFAULT_SORT_BY, DEFAULT_SORT_DIR);
}
