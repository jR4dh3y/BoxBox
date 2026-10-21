import { useBrowseSelection } from './selection.svelte';
import {
	parseBrowseLocation,
	useBrowseLocation,
	type BrowseUrl,
	type BrowseLocationSnapshot
} from './location.svelte';
import type { BrowseActions } from './actions.svelte';
import type { BrowseData } from './data.svelte';
import type { useBrowseUploads } from './uploads.svelte';

export interface BrowseLaunchState {
	location: BrowseLocationSnapshot;
	selectPath: string | null;
}

export function parseBrowseLaunchState(url: BrowseUrl): BrowseLaunchState {
	const selectPath = normalizeSelectedPath(url.searchParams.get('select'));
	const location = parseBrowseLocation(url);
	if (!selectPath) return { location, selectPath: null };

	const parentPath = selectPath.slice(0, Math.max(0, selectPath.lastIndexOf('/')));
	return {
		location: { ...location, path: parentPath, searchQuery: '' },
		selectPath
	};
}

function normalizeSelectedPath(value: string | null): string | null {
	if (value === null) return null;
	const path = value.replace(/^\/+|\/+$/g, '');
	if (!path || path.split('/').some((segment) => segment === '.' || segment === '..')) return null;
	return path;
}

export function createBrowsePaneState(
	id: string,
	initialLocation: BrowseLocationSnapshot,
	onLocationChange?: (snapshot: BrowseLocationSnapshot) => void
) {
	const selection = useBrowseSelection();
	const location = useBrowseLocation({
		initial: initialLocation,
		onLocationChange: () => selection.clear(),
		onChange: onLocationChange
	});

	return { id, location, selection };
}

export type BrowsePaneState = ReturnType<typeof createBrowsePaneState>;

export interface BrowsePaneRuntime {
	pane: BrowsePaneState;
	data: BrowseData;
	actions: BrowseActions;
	uploads: ReturnType<typeof useBrowseUploads>;
	hasOpenDialog: boolean;
}
