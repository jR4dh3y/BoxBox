import { isLocalDesktop } from '#lib/api/desktop.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import type { SortDir, SortField, ViewMode } from '#lib/types/files.js';
import { parseRemotePath } from '#lib/api/remotePaths.js';
function remotePath(path: string) {
	const source = parseRemotePath(path);
	return source.kind === 'remote' ? { id: source.serverId, path: source.path } : null;
}
function serverPath(id: string, path: string) {
	return `@remote/${id}${path ? '/' + path : ''}`;
}

/** A pane shows a folder or one of the desktop app's own pages. */
export type BrowsePage = 'files' | 'trash';

export interface BrowseLocationSnapshot {
	page: BrowsePage;
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
	const normalized = path.replace(/^\/+|\/+$/g, '');
	try {
		parseRemotePath(normalized);
		return normalized;
	} catch {
		return '';
	}
}

function normalizeSnapshot(snapshot: BrowseLocationSnapshot): BrowseLocationSnapshot {
	if (snapshot.page !== 'files') return { ...snapshot, path: '', searchQuery: '' };
	return { ...snapshot, path: normalizePath(snapshot.path) };
}

/** Pages exist only in the desktop app; the web app always shows files. */
function parsePage(value: string | null): BrowsePage {
	if (!isLocalDesktop()) return 'files';
	return value === 'trash' ? value : 'files';
}

export function parseBrowseLocation(url: BrowseUrl): BrowseLocationSnapshot {
	const path = normalizePath(url.searchParams.get('path') ?? '');
	const atRoot = !path || /^@remote\/[^/]+$/.test(path);
	return normalizeSnapshot({
		page: parsePage(url.searchParams.get('page')),
		path,
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
					: atRoot
						? 'grid'
						: settingsStore.current.defaultViewMode
	});
}

function sameSnapshot(left: BrowseLocationSnapshot, right: BrowseLocationSnapshot): boolean {
	return (
		left.page === right.page &&
		left.path === right.path &&
		left.searchQuery === right.searchQuery &&
		left.sortBy === right.sortBy &&
		left.sortDir === right.sortDir &&
		left.viewMode === right.viewMode
	);
}

/** A pane's location and folder history. Search and view changes replace the current entry. */
export function usePaneLocation(options: BrowseLocationOptions = {}) {
	const initial = normalizeSnapshot(
		options.initial ?? {
			page: 'files',
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
	const canGoUp = $derived(
		snapshot.page === 'files' &&
			(remotePath(path) ? remotePath(path)?.path !== '' : segments.length > 0)
	);

	function changed(previous: BrowseLocationSnapshot, next: BrowseLocationSnapshot): void {
		if (previous.page !== next.page || previous.path !== next.path) options.onLocationChange?.();
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
		const atRoot = !newPath || /^@remote\/[^/]+$/.test(newPath);
		const wasRoot = !path || /^@remote\/[^/]+$/.test(path);
		push({
			...snapshot,
			page: 'files',
			path: newPath,
			searchQuery: '',
			viewMode: atRoot
				? 'grid'
				: wasRoot
					? settingsStore.current.defaultViewMode
					: snapshot.viewMode
		});
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
		get page() {
			return snapshot.page;
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
		get viewKey() {
			return JSON.stringify([snapshot, settingsStore.current.showHiddenFiles]);
		},
		get driveViewMode() {
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
		openPage(page: BrowsePage) {
			push({ ...snapshot, page, path: '', searchQuery: '' });
		},
		restore(snapshot: BrowseLocationSnapshot) {
			const normalized = normalizeSnapshot(snapshot);
			const index = history.findIndex((entry) => sameSnapshot(entry, normalized));
			if (index >= 0) moveHistory(index);
			else replaceCurrent(normalized);
		},
		back() {
			if (canGoBack) moveHistory(historyIndex - 1);
		},
		forward() {
			if (canGoForward) moveHistory(historyIndex + 1);
		},
		up() {
			const remote = remotePath(path);
			if (canGoUp)
				navigate(
					remote
						? serverPath(remote.id, remote.path.split('/').slice(0, -1).join('/'))
						: segments.slice(0, -1).join('/')
				);
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
