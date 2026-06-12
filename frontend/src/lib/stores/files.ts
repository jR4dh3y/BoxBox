/**
 * Files store for managing file listing state
 * Requirements: 1.1, 1.2
 */

import { writable, derived, get } from 'svelte/store';
import { type ListOptions } from '$lib/api/files';
import { CONFIG } from '$lib/config';

/**
 * Current path state
 */
export interface PathState {
	currentPath: string;
	pathSegments: string[];
}

/**
 * Create the path store
 */
function createPathStore() {
	const { subscribe, set, update } = writable<PathState>({
		currentPath: '',
		pathSegments: []
	});

	/**
	 * Navigate to a path
	 */
	function navigateTo(path: string): void {
		const normalizedPath = path.replace(/^\/+|\/+$/g, '');
		const segments = normalizedPath ? normalizedPath.split('/') : [];
		set({
			currentPath: normalizedPath,
			pathSegments: segments
		});
	}

	/**
	 * Navigate up one level
	 */
	function navigateUp(): void {
		update((state) => {
			if (state.pathSegments.length === 0) return state;
			const newSegments = state.pathSegments.slice(0, -1);
			return {
				currentPath: newSegments.join('/'),
				pathSegments: newSegments
			};
		});
	}

	/**
	 * Navigate to root
	 */
	function navigateToRoot(): void {
		set({
			currentPath: '',
			pathSegments: []
		});
	}

	/**
	 * Get current path
	 */
	function getCurrentPath(): string {
		return get({ subscribe }).currentPath;
	}

	return {
		subscribe,
		navigateTo,
		navigateUp,
		navigateToRoot,
		getCurrentPath
	};
}

/**
 * Path store singleton
 */
export const pathStore = createPathStore();

/**
 * Derived store for current path string
 */
export const currentPath = derived(pathStore, ($path) => $path.currentPath);

/**
 * Derived store for path segments (for breadcrumbs)
 */
export const pathSegments = derived(pathStore, ($path) => $path.pathSegments);

/**
 * List options state
 */
export interface ListOptionsState extends ListOptions {
	page: number;
	pageSize: number;
	sortBy: 'name' | 'size' | 'modTime' | 'type';
	sortDir: 'asc' | 'desc';
	filter: string;
}

/**
 * Default list options
 */
const defaultListOptions: ListOptionsState = {
	page: 1,
	pageSize: CONFIG.ui.defaultPageSize,
	sortBy: 'name',
	sortDir: 'asc',
	filter: ''
};

/**
 * Create the list options store
 */
function createListOptionsStore() {
	const { subscribe, set, update } = writable<ListOptionsState>(defaultListOptions);

	function setPage(page: number): void {
		update((state) => ({ ...state, page }));
	}

	function setPageSize(pageSize: number): void {
		update((state) => ({ ...state, pageSize, page: 1 }));
	}

	function setSortBy(sortBy: ListOptionsState['sortBy']): void {
		update((state) => ({ ...state, sortBy, page: 1 }));
	}

	function setSortDir(sortDir: ListOptionsState['sortDir']): void {
		update((state) => ({ ...state, sortDir, page: 1 }));
	}

	function toggleSortDir(): void {
		update((state) => ({
			...state,
			sortDir: state.sortDir === 'asc' ? 'desc' : 'asc',
			page: 1
		}));
	}

	function setFilter(filter: string): void {
		update((state) => ({ ...state, filter, page: 1 }));
	}

	function reset(): void {
		set(defaultListOptions);
	}

	function getOptions(): ListOptionsState {
		return get({ subscribe });
	}

	return {
		subscribe,
		setPage,
		setPageSize,
		setSortBy,
		setSortDir,
		toggleSortDir,
		setFilter,
		reset,
		getOptions
	};
}

/**
 * List options store singleton
 */
export const listOptionsStore = createListOptionsStore();

/**
 * Query key factory for files
 */
export const fileQueryKeys = {
	all: ['files'] as const,
	roots: () => [...fileQueryKeys.all, 'roots'] as const,
	list: (path: string, options: ListOptions) =>
		[...fileQueryKeys.all, 'list', path, options] as const,
	search: (path: string, query: string) => [...fileQueryKeys.all, 'search', path, query] as const
};
