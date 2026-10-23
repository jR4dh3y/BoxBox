import type { BrowsePage } from './location.svelte';
import { rootLabel } from './rootLabel';

const PAGE_TITLES: Record<Exclude<BrowsePage, 'files'>, string> = {
	trash: 'Trash',
	drives: 'Removable drives',
	servers: 'Servers'
};

/** The name of a page, or of the root; folders are named by their path. */
export function locationTitle(location: { page: BrowsePage; path: string }): string {
	if (location.page !== 'files') return PAGE_TITLES[location.page];
	return location.path || rootLabel();
}
