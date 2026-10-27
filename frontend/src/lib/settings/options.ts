import type { UserSettings } from '#lib/stores/settings.svelte.js';

export const SORT_BY_OPTIONS: { value: UserSettings['defaultSortBy']; label: string }[] = [
	{ value: 'name', label: 'Name' },
	{ value: 'size', label: 'Size' },
	{ value: 'modTime', label: 'Date modified' },
	{ value: 'type', label: 'Type' }
];

export const SORT_DIR_OPTIONS: { value: UserSettings['defaultSortDir']; label: string }[] = [
	{ value: 'asc', label: 'Ascending' },
	{ value: 'desc', label: 'Descending' }
];

export const VIEW_MODE_OPTIONS: { value: UserSettings['defaultViewMode']; label: string }[] = [
	{ value: 'list', label: 'List' },
	{ value: 'grid', label: 'Grid' }
];

/** Narrows a string from a <select> to one of its options, without a cast. */
export function pickOption<T extends string>(
	options: readonly { value: T }[],
	value: string
): T | undefined {
	return options.find((option) => option.value === value)?.value;
}
