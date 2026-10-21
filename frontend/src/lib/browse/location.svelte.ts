import { settingsStore } from '#lib/stores/settings.svelte.js';
import type { SortDir, SortField, ViewMode } from '#lib/types/files.js';

export interface BrowseLocationSnapshot {
	path: string;
	searchQuery: string;
	sortBy: SortField;
	sortDir: SortDir;
	viewMode: ViewMode;
}

interface BrowseLocationOptions {
	initial?: BrowseLocationSnapshot;
	onLocationChange?: () => void;
	onChange?: (snapshot: BrowseLocationSnapshot) => void;
}

export interface BrowseUrl {
	searchParams: { get(name: string): string | null };
}

function parseSortField(value: string | null): SortField {
	if (value === 'name' || value === 'size' || value === 'modTime' || value === 'type') return value;
	return settingsStore.current.defaultSortBy;
}

function normalizePath(path: string): string {
	return path.replace(/^\/+|\/+$/g, '');
}

function normalizeSnapshot(snapshot: BrowseLocationSnapshot): BrowseLocationSnapshot {
	return { ...snapshot, path: normalizePath(snapshot.path) };
}

export function parseBrowseLocation(url: BrowseUrl): BrowseLocationSnapshot {
	return normalizeSnapshot({
		path: url.searchParams.get('path') ?? '',
		searchQuery: url.searchParams.get('q') ?? '',
		sortBy: parseSortField(url.searchParams.get('sort')),
		sortDir:
			url.searchParams.get('dir') === 'desc'
				? 'desc'
				: url.searchParams.get('dir') === 'asc'
					? 'asc'
					: settingsStore.current.defaultSortDir,
		viewMode:
			url.searchParams.get('view') === 'grid'
				? 'grid'
				: url.searchParams.get('view') === 'list'
					? 'list'
					: settingsStore.current.defaultViewMode
	});
}

function sameSnapshot(left: BrowseLocationSnapshot, right: BrowseLocationSnapshot): boolean {
	return (
		left.path === right.path &&
		left.searchQuery === right.searchQuery &&
		left.sortBy === right.sortBy &&
		left.sortDir === right.sortDir &&
		left.viewMode === right.viewMode
	);
}

/** A pane's location and folder history. Search and view changes replace the current entry. */
export function useBrowseLocation(options: BrowseLocationOptions = {}) {
	const initial = normalizeSnapshot(
		options.initial ?? {
			path: '',
			searchQuery: '',
			sortBy: settingsStore.current.defaultSortBy,
			sortDir: settingsStore.current.defaultSortDir,
			viewMode: settingsStore.current.defaultViewMode
		}
	);
	let history = $state.raw<BrowseLocationSnapshot[]>([initial]);
	let historyIndex = $state(0);

	const snapshot = $derived(history[historyIndex] ?? initial);
	const path = $derived(snapshot.path);
	const segments = $derived(path ? path.split('/') : []);
	const trimmedSearchQuery = $derived(snapshot.searchQuery.trim());
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

	function changed(previous: BrowseLocationSnapshot, next: BrowseLocationSnapshot): void {
		if (previous.path !== next.path) options.onLocationChange?.();
		options.onChange?.(next);
	}

	function replaceCurrent(next: BrowseLocationSnapshot): void {
		const previous = snapshot;
		const normalized = normalizeSnapshot(next);
		if (sameSnapshot(previous, normalized)) return;
		history = history.map((entry, index) => (index === historyIndex ? normalized : entry));
		changed(previous, normalized);
	}

	function push(next: BrowseLocationSnapshot): void {
		const previous = snapshot;
		const normalized = normalizeSnapshot(next);
		if (sameSnapshot(previous, normalized)) return;
		history = [...history.slice(0, historyIndex + 1), normalized];
		historyIndex = history.length - 1;
		changed(previous, normalized);
	}

	function navigate(newPath: string): void {
		push({ ...snapshot, path: newPath, searchQuery: '' });
	}

	function moveHistory(nextIndex: number): void {
		const previous = snapshot;
		historyIndex = nextIndex;
		const next = snapshot;
		changed(previous, next);
	}

	return {
		get snapshot() {
			return snapshot;
		},
		get path() {
			return path;
		},
		get segments() {
			return segments;
		},
		get searchQuery() {
			return snapshot.searchQuery;
		},
		get trimmedSearchQuery() {
			return trimmedSearchQuery;
		},
		get isSearchActive() {
			return isSearchActive;
		},
		get sortBy() {
			return snapshot.sortBy;
		},
		get sortDir() {
			return snapshot.sortDir;
		},
		get viewMode() {
			return snapshot.viewMode;
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
