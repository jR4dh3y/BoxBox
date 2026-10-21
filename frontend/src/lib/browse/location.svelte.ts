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
	const canGoBack = $derived(historyIndex > 0);
	const canGoForward = $derived(historyIndex < history.length - 1);
	const canGoUp = $derived(segments.length > 0);

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
		get canGoBack() {
			return canGoBack;
		},
		get canGoForward() {
			return canGoForward;
		},
		get canGoUp() {
			return canGoUp;
		},
		navigate,
		back() {
			if (canGoBack) moveHistory(historyIndex - 1);
		},
		forward() {
			if (canGoForward) moveHistory(historyIndex + 1);
		},
		up() {
			if (canGoUp) navigate(segments.slice(0, -1).join('/'));
		},
		search(query: string) {
			replaceCurrent({ ...snapshot, searchQuery: query.trim() });
		},
		clearSearch() {
			replaceCurrent({ ...snapshot, searchQuery: '' });
		},
		setSort(sortBy: SortField, sortDir: SortDir) {
			replaceCurrent({ ...snapshot, sortBy, sortDir });
		},
		setViewMode(viewMode: ViewMode) {
			replaceCurrent({ ...snapshot, viewMode });
		}
	};
}

export type BrowseLocation = ReturnType<typeof useBrowseLocation>;
