import { afterNavigate, goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { page } from '$app/state';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import type { SortDir, SortField, ViewMode } from '#lib/types/files.js';

function parseSortField(value: string | null): SortField {
	if (value === 'name' || value === 'size' || value === 'modTime' || value === 'type') return value;
	return settingsStore.current.defaultSortBy;
}

/**
 * Where the user is in the file browser. The URL holds it: `path`, `q`, `sort`, `dir` and `view`.
 * Call during component setup. `onLocationChange` runs when the folder changes, by link or by back/forward.
 */
export function useBrowseLocation(options: { onLocationChange: () => void }) {
	let historyMaxIndex = $state(page.state.browseHistoryIndex ?? 0);

	const path = $derived(page.url.searchParams.get('path')?.replace(/^\/+|\/+$/g, '') ?? '');
	const segments = $derived(path ? path.split('/') : []);
	const searchQuery = $derived(page.url.searchParams.get('q') ?? '');
	const trimmedSearchQuery = $derived(searchQuery.trim());
	const isSearchActive = $derived(trimmedSearchQuery.length >= 2);
	const sortBy = $derived<SortField>(parseSortField(page.url.searchParams.get('sort')));
	const sortDir = $derived<SortDir>(
		page.url.searchParams.get('dir') === 'desc'
			? 'desc'
			: page.url.searchParams.get('dir') === 'asc'
				? 'asc'
				: settingsStore.current.defaultSortDir
	);
	const viewMode = $derived<ViewMode>(
		page.url.searchParams.get('view') === 'grid'
			? 'grid'
			: page.url.searchParams.get('view') === 'list'
				? 'list'
				: settingsStore.current.defaultViewMode
	);
	// The drive cards are the drives page's own look, so only an explicit choice shows the list.
	const driveViewMode = $derived<ViewMode>(
		page.url.searchParams.get('view') === 'list' ? 'list' : 'grid'
	);
	const viewKey = $derived(
		JSON.stringify([
			path,
			isSearchActive ? trimmedSearchQuery : '',
			sortBy,
			sortDir,
			viewMode,
			settingsStore.current.showHiddenFiles
		])
	);
	const historyIndex = $derived(page.state.browseHistoryIndex ?? 0);
	const canGoBack = $derived(historyIndex > 0);
	const canGoForward = $derived(historyIndex < historyMaxIndex);

	afterNavigate((navigation) => {
		if (navigation.type === 'popstate') options.onLocationChange();
	});

	/** Search, sort and view changes replace the history entry. A new folder adds one. */
	function updateParams(
		updates: Record<string, string | null>,
		{ replaceState = false }: { replaceState?: boolean } = {}
	) {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- Only the navigation target string escapes this handler.
		const url = new URL(page.url.href);
		for (const [key, value] of Object.entries(updates)) {
			if (value) url.searchParams.set(key, value);
			else url.searchParams.delete(key);
		}
		const target = `${resolve('browse')}${url.search}${url.hash}`;

		if (replaceState) {
			void goto(target, {
				replace: true,
				reset: false,
				state: page.state
			});
			return;
		}

		const nextIndex = historyIndex + 1;
		historyMaxIndex = nextIndex;
		void goto(target, {
			reset: false,
			state: { ...page.state, browseHistoryIndex: nextIndex }
		});
	}

	function navigate(newPath: string) {
		options.onLocationChange();
		updateParams({ path: newPath || null, q: null });
	}

	return {
		get path() {
			return path;
		},
		get segments() {
			return segments;
		},
		get searchQuery() {
			return searchQuery;
		},
		get trimmedSearchQuery() {
			return trimmedSearchQuery;
		},
		get isSearchActive() {
			return isSearchActive;
		},
		get sortBy() {
			return sortBy;
		},
		get sortDir() {
			return sortDir;
		},
		get viewMode() {
			return viewMode;
		},
		get driveViewMode() {
			return driveViewMode;
		},
		get viewKey() {
			return viewKey;
		},
		get canGoBack() {
			return canGoBack;
		},
		get canGoForward() {
			return canGoForward;
		},
		navigate,
		back: () => history.back(),
		forward: () => history.forward(),
		search: (query: string) => updateParams({ q: query.trim() || null }, { replaceState: true }),
		clearSearch: () => updateParams({ q: null }, { replaceState: true }),
		setSort: (field: SortField, dir: SortDir) =>
			updateParams({ sort: field, dir }, { replaceState: true }),
		setViewMode: (mode: ViewMode) => updateParams({ view: mode }, { replaceState: true })
	};
}

export type BrowseLocation = ReturnType<typeof useBrowseLocation>;
