import { afterNavigate, beforeNavigate, goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { page } from '$app/state';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import type { SortDir, SortField, ViewMode } from '#lib/types/files.js';
import { folderExists } from './folderExists.js';
import { lastLocation, rememberLocation } from './startLocation.js';
import { onMount } from 'svelte';
import { folderView, isRootPath } from './folderViews.js';
import { HistoryTrail } from './historyTrail.js';

export interface HistoryPlace {
	/** The history entry, for `travelTo`. */
	index: number;
	path: string;
}

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
export function useBrowseLocation(options: {
	onLocationChange: () => void;
	/** What is selected, remembered as the page leaves a history entry. */
	selection?: () => ReadonlySet<string>;
	/** Back and Forward put back what was selected in the entry they return to. */
	onRestoreSelection?: (paths: readonly string[]) => void;
}) {
	let historyMaxIndex = $state(page.state.browseHistoryIndex ?? 0);
	const trail = new HistoryTrail(typeof sessionStorage === 'undefined' ? null : sessionStorage);

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

	// An app opened without a folder in its address starts where Settings say; a folder that
	// is gone leaves it on the drives.
	const startAt = page.url.searchParams.has('path') ? null : lastLocation();
	onMount(() => {
		if (!startAt || settingsStore.current.startLocation !== 'last' || historyIndex !== 0) return;
		void folderExists(startAt).then((found) => {
			if (found && !page.url.searchParams.has('path'))
				updateParams({ path: startAt }, { replaceState: true });
		});
	});
	let arrivedAt = page.state.browseHistoryIndex ?? 0;
	trail.arrive(arrivedAt, path, false);
	rememberLocation(path);
	beforeNavigate((navigation) => {
		// Back, Forward and links leave the entry here; `navigate` already did for itself.
		if (navigation.type !== 'goto' && options.selection)
			trail.leave(historyIndex, options.selection());
	});
	afterNavigate((navigation) => {
		const index = page.state.browseHistoryIndex ?? 0;
		trail.arrive(index, path, navigation.type !== 'popstate' && index > arrivedAt);
		arrivedAt = index;
		rememberLocation(path);
		if (navigation.type !== 'popstate') return;
		options.onLocationChange();
		const selected = trail.at(index)?.selected ?? [];
		if (selected.length > 0) options.onRestoreSelection?.(selected);
	});

	/** Moves to the nearest place back or forward that still exists, as Thunar does. */
	async function travel(back: boolean): Promise<void> {
		const from = historyIndex;
		const next = back ? from - 1 : from + 1;
		if (next < 0 || next > historyMaxIndex) return;
		const places = trail.around(from, back, historyMaxIndex);
		// An entry from before the trail began is not known; the browser takes the step.
		if (places[0]?.index !== next) {
			history.go(back ? -1 : 1);
			return;
		}
		for (const place of places) {
			if (await folderExists(place.entry.path)) {
				if (historyIndex === from) history.go(place.index - from);
				return;
			}
		}
	}

	/** The places in a Back or Forward menu that still exist, nearest first. */
	async function historyPlaces(back: boolean): Promise<HistoryPlace[]> {
		const places = trail.around(historyIndex, back, historyMaxIndex);
		const found = await Promise.all(places.map(({ entry }) => folderExists(entry.path)));
		return places
			.filter((_, at) => found[at])
			.map(({ index, entry }) => ({ index, path: entry.path }));
	}

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
		// Remembered before the folder change clears it; Back brings it back.
		if (options.selection) trail.leave(historyIndex, options.selection());
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
		back(): void {
			void travel(true);
		},
		forward(): void {
			void travel(false);
		},
		historyPlaces,
		/** Goes to a place a Back or Forward menu listed. */
		travelTo(index: number) {
			if (index !== historyIndex) history.go(index - historyIndex);
		},
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
