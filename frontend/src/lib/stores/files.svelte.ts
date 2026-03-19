/**
 * Files-related state and query helpers using Svelte 5 runes.
 */

import {
	listRoots,
	listDirectory,
	createDirectory,
	rename,
	deleteFile,
	search,
	type ListOptions
} from '$lib/api/files';
import { SvelteSet } from 'svelte/reactivity';

export interface PathState {
	currentPath: string;
	pathSegments: string[];
}

class PathStore {
	private state = $state<PathState>({
		currentPath: '',
		pathSegments: []
	});

	get currentPath(): string {
		return this.state.currentPath;
	}

	get pathSegments(): string[] {
		return this.state.pathSegments;
	}

	navigateTo(path: string): void {
		const normalizedPath = path.replace(/^\/+|\/+$/g, '');
		this.state.currentPath = normalizedPath;
		this.state.pathSegments = normalizedPath ? normalizedPath.split('/') : [];
	}

	navigateUp(): void {
		if (this.state.pathSegments.length === 0) {
			return;
		}
		const nextSegments = this.state.pathSegments.slice(0, -1);
		this.state.pathSegments = nextSegments;
		this.state.currentPath = nextSegments.join('/');
	}

	navigateToRoot(): void {
		this.state.currentPath = '';
		this.state.pathSegments = [];
	}

	getCurrentPath(): string {
		return this.state.currentPath;
	}
}

export const pathStore = new PathStore();

export interface ListOptionsState extends ListOptions {
	page: number;
	pageSize: number;
	sortBy: 'name' | 'size' | 'modTime' | 'type';
	sortDir: 'asc' | 'desc';
	filter: string;
}

const defaultListOptions: ListOptionsState = {
	page: 1,
	pageSize: 50,
	sortBy: 'name',
	sortDir: 'asc',
	filter: ''
};

class ListOptionsStore {
	private state = $state<ListOptionsState>({ ...defaultListOptions });

	get value(): ListOptionsState {
		return this.state;
	}

	setPage(page: number): void {
		this.state.page = page;
	}

	setPageSize(pageSize: number): void {
		this.state.pageSize = pageSize;
		this.state.page = 1;
	}

	setSortBy(sortBy: ListOptionsState['sortBy']): void {
		this.state.sortBy = sortBy;
		this.state.page = 1;
	}

	setSortDir(sortDir: ListOptionsState['sortDir']): void {
		this.state.sortDir = sortDir;
		this.state.page = 1;
	}

	toggleSortDir(): void {
		this.state.sortDir = this.state.sortDir === 'asc' ? 'desc' : 'asc';
		this.state.page = 1;
	}

	setFilter(filter: string): void {
		this.state.filter = filter;
		this.state.page = 1;
	}

	reset(): void {
		this.state = { ...defaultListOptions };
	}

	getOptions(): ListOptionsState {
		return this.state;
	}
}

export const listOptionsStore = new ListOptionsStore();

export interface SelectionState {
	selectedItems: SvelteSet<string>;
	lastSelectedItem: string | null;
}

class SelectionStore {
	private state = $state<SelectionState>({
		selectedItems: new SvelteSet<string>(),
		lastSelectedItem: null
	});

	get selectedItems(): SvelteSet<string> {
		return this.state.selectedItems;
	}

	get lastSelectedItem(): string | null {
		return this.state.lastSelectedItem;
	}

	get selectedCount(): number {
		return this.state.selectedItems.size;
	}

	get hasSelection(): boolean {
		return this.state.selectedItems.size > 0;
	}

	select(path: string): void {
		const next = new SvelteSet(this.state.selectedItems);
		next.add(path);
		this.state.selectedItems = next;
		this.state.lastSelectedItem = path;
	}

	deselect(path: string): void {
		const next = new SvelteSet(this.state.selectedItems);
		next.delete(path);
		this.state.selectedItems = next;
	}

	toggle(path: string): void {
		const next = new SvelteSet(this.state.selectedItems);
		if (next.has(path)) {
			next.delete(path);
		} else {
			next.add(path);
		}
		this.state.selectedItems = next;
		this.state.lastSelectedItem = path;
	}

	selectOnly(path: string): void {
		this.state.selectedItems = new SvelteSet([path]);
		this.state.lastSelectedItem = path;
	}

	selectAll(paths: string[]): void {
		this.state.selectedItems = new SvelteSet(paths);
		this.state.lastSelectedItem = paths[paths.length - 1] ?? null;
	}

	clearSelection(): void {
		this.state.selectedItems = new SvelteSet();
		this.state.lastSelectedItem = null;
	}

	isSelected(path: string): boolean {
		return this.state.selectedItems.has(path);
	}

	getSelectedItems(): string[] {
		return Array.from(this.state.selectedItems);
	}
}

export const selectionStore = new SelectionStore();

export const fileQueryKeys = {
	all: ['files'] as const,
	roots: () => [...fileQueryKeys.all, 'roots'] as const,
	list: (path: string, options: ListOptions) =>
		[...fileQueryKeys.all, 'list', path, options] as const,
	search: (path: string, query: string) => [...fileQueryKeys.all, 'search', path, query] as const
};

export function rootsQueryOptions() {
	return {
		queryKey: fileQueryKeys.roots(),
		queryFn: () => listRoots()
	};
}

export function directoryQueryOptions(path: string, options: ListOptions) {
	return {
		queryKey: fileQueryKeys.list(path, options),
		queryFn: () => listDirectory(path, options),
		enabled: path !== ''
	};
}

export function searchQueryOptions(path: string, query: string) {
	return {
		queryKey: fileQueryKeys.search(path, query),
		queryFn: () => search(path, query),
		enabled: query.length > 0
	};
}

export function createDirectoryMutationOptions() {
	return {
		mutationFn: ({ basePath, name }: { basePath: string; name: string }) =>
			createDirectory(basePath, name)
	};
}

export function renameMutationOptions() {
	return {
		mutationFn: ({ oldPath, newPath }: { oldPath: string; newPath: string }) =>
			rename(oldPath, newPath)
	};
}

export function deleteMutationOptions() {
	return {
		mutationFn: ({ path, confirm }: { path: string; confirm?: boolean }) =>
			deleteFile(path, confirm)
	};
}
