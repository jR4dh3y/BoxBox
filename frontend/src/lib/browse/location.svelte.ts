import { afterNavigate, goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { page } from '$app/state';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import type { SortDir, SortField, ViewMode } from '#lib/types/files.js';
import { folderView, isRootPath } from './folderViews.js';

function parseSortField(value: string | null): SortField | null {
	if (value === 'name' || value === 'size' || value === 'modTime' || value === 'type') return value;
	return null;
}

function parseSortDir(value: string | null): SortDir | null {
	return value === 'asc' || value === 'desc' ? value : null;
}

function parseViewMode(value: string | null): ViewMode | null {
	return value === 'list' || value === 'grid' ? value : null;
}

/**
 * Where the user is in the file browser. The URL holds it: `path`, `q`, `sort`, `dir` and `view`.
 * A folder opened without `sort`, `dir` or `view` gets the ones remembered for it.
 * Call during component setup. `onLocationChange` runs when the folder changes, by link or by back/forward.
 */
export function useBrowseLocation(options: { onLocationChange: () => void }) {
	let historyMaxIndex = $state(page.state.browseHistoryIndex ?? 0);

	const path = $derived(page.url.searchParams.get('path')?.replace(/^\/+|\/+$/g, '') ?? '');
	const segments = $derived(path ? path.split('/') : []);
	const searchQuery = $derived(page.url.searchParams.get('q') ?? '');
	const trimmedSearchQuery = $derived(searchQuery.trim());
	const isSearchActive = $derived(trimmedSearchQuery.length >= 2);
	const opening = $derived.by(() => {
		const settings = settingsStore.current;
		return folderView(settings.folderViews, path, {
			// The drive cards are the drives page's own look until a view is picked there.
			mode: isRootPath(path) ? 'grid' : settings.defaultViewMode,
			sort: { by: settings.defaultSortBy, dir: settings.defaultSortDir },
			perFolder: settings.viewPerFolder || isRootPath(path)
		});
	});
	const sortBy = $derived(parseSortField(page.url.searchParams.get('sort')) ?? opening.sort.by);
	const sortDir = $derived(parseSortDir(page.url.searchParams.get('dir')) ?? opening.sort.dir);
	const viewMode = $derived(parseViewMode(page.url.searchParams.get('view')) ?? opening.mode);
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

	/** The new folder opens in its own view and sort, so the old folder's are dropped. */
	function navigate(newPath: string) {
		options.onLocationChange();
		updateParams({ path: newPath || null, q: null, sort: null, dir: null, view: null });
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
			return viewMode;
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
		/** A folder keeps the sort picked in it; search results and the drives are not a folder. */
		setSort: (field: SortField, dir: SortDir) => {
			updateParams({ sort: field, dir }, { replaceState: true });
			if (!isRootPath(path) && !isSearchActive)
				settingsStore.chooseFolderSort(path, { by: field, dir });
		},
		setViewMode: (mode: ViewMode) => {
			updateParams({ view: mode }, { replaceState: true });
			settingsStore.chooseFolderMode(path, mode);
		}
	};
}

export type BrowseLocation = ReturnType<typeof useBrowseLocation>;
