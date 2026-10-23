import { createInfiniteQuery, createQuery, useQueryClient } from '@tanstack/svelte-query';
import {
	getDriveStats,
	listRoots,
	search,
	type DriveStatsResponse,
	type FileInfo,
	type ListOptions,
	type RootsResponse,
	type SearchResponse
} from '#lib/api/files.js';
import { CONFIG } from '#lib/config.js';
import { fileQueryKeys } from '#lib/stores/files.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { canPreview } from '#lib/utils/fileTypes.js';
import { isNativePath } from '#lib/api/desktop.js';
import { isRemotePath } from '#lib/api/remotePaths.js';
import { watchDesktopDirectory } from './desktopWatch';
import { directoryQueryOptions } from './directoryQuery';
import type { BrowseLocation } from './location.svelte';

/**
 * What the browser shows for the current location: drives, a folder page or search results.
 * Call during component setup because the queries need the component context.
 */
export function useBrowseData(location: BrowseLocation, revealHidden: () => boolean = () => false) {
	const queryClient = useQueryClient();
	const settings = $derived(settingsStore.current);
	const includeHidden = $derived(settings.showHiddenFiles || revealHidden());

	const directoryOptions = $derived<Omit<ListOptions, 'page'>>({
		pageSize: 50,
		sortBy: location.sortBy,
		sortDir: location.sortDir,
		includeHidden
	});

	const rootsQuery = createQuery<RootsResponse>(() => ({
		queryKey: fileQueryKeys.roots(),
		queryFn: () => listRoots(),
		staleTime: 0,
		refetchOnWindowFocus: 'always',
		refetchInterval: 15_000
	}));

	const driveStatsQuery = createQuery<DriveStatsResponse>(() => ({
		queryKey: [...fileQueryKeys.roots(), 'stats'],
		queryFn: () => getDriveStats(),
		enabled: location.page === 'files' && location.path === ''
	}));

	const directoryQuery = createInfiniteQuery(() => ({
		...directoryQueryOptions(location.path, directoryOptions),
		enabled: location.path !== '',
		refetchOnWindowFocus: 'always',
		refetchInterval: isRemotePath(location.path) ? 15_000 : false
	}));

	const searchQueryResult = createQuery<SearchResponse>(() => ({
		queryKey: fileQueryKeys.search(location.path, location.trimmedSearchQuery),
		queryFn: ({ signal }) => search(location.path, location.trimmedSearchQuery, signal),
		enabled: location.path !== '' && location.isSearchActive,
		refetchOnWindowFocus: 'always',
		refetchInterval: isRemotePath(location.path) ? 15_000 : false
	}));

	const isAtRoot = $derived(location.page === 'files' && location.path === '');
	const inFolder = $derived(location.page === 'files' && location.path !== '');
	// Pages load their own data; reloading one remounts it.
	let pageRevision = $state(0);
	const roots = $derived(rootsQuery.data?.roots ?? []);
	const drives = $derived(driveStatsQuery.data?.drives ?? []);
	const directoryPages = $derived(directoryQuery.data?.pages ?? []);
	const directoryItems = $derived(directoryPages.flatMap((directoryPage) => directoryPage.items));
	const searchResults = $derived(searchQueryResult.data?.results ?? []);

	const displayItems = $derived.by(() => {
		const items: FileInfo[] = location.isSearchActive ? searchResults : directoryItems;
		return includeHidden ? items : items.filter((item) => !item.name.startsWith('.'));
	});

	function mountOf(path: string) {
		if (!path) return null;
		return roots.find((root) => path === root.name || path.startsWith(`${root.name}/`)) ?? null;
	}
	const isReadOnly = $derived(mountOf(location.path)?.readOnly ?? true);
	const hasMoreItems = $derived(
		!location.isSearchActive && !isAtRoot && directoryQuery.hasNextPage
	);
	$effect(() => {
		const path = location.path;
		if (!isNativePath(path) || !path) return;
		return watchDesktopDirectory(path, () => {
			void queryClient.invalidateQueries({ queryKey: fileQueryKeys.directory(path) });
			void queryClient.invalidateQueries({ queryKey: fileQueryKeys.searches() });
		});
	});

	return {
		get roots() {
			return roots;
		},
		get drives() {
			return drives;
		},
		get isAtRoot() {
			return isAtRoot;
		},
		get isLoadingDrives() {
			return rootsQuery.isLoading || driveStatsQuery.isLoading;
		},
		get isSearching() {
			return location.isSearchActive && searchQueryResult.isFetching;
		},
		get isFileListLoading() {
			return location.isSearchActive
				? searchQueryResult.isFetching
				: directoryQuery.isLoading && directoryItems.length === 0;
		},
		get error() {
			const error = location.isSearchActive ? searchQueryResult.error : directoryQuery.error;
			return error instanceof Error ? error.message : null;
		},
		get isLoadingMore() {
			return !location.isSearchActive && directoryQuery.isFetchingNextPage;
		},
		get displayItems() {
			return displayItems;
		},
		get previewableFiles() {
			return displayItems.filter((item) => !item.isDir && canPreview(item.name));
		},
		get hasMoreItems() {
			return hasMoreItems;
		},
		get statusTotalCount() {
			return isAtRoot || location.isSearchActive ? undefined : (directoryPages[0]?.totalCount ?? 0);
		},
		get emptyListMessage() {
			return location.isSearchActive
				? `No matches for "${location.trimmedSearchQuery}" in this folder`
				: 'This folder is empty';
		},
		get itemCount() {
			return isAtRoot ? drives.length : displayItems.length;
		},
		get isReadOnly() {
			return isReadOnly;
		},
		get canCreate() {
			return inFolder && !isReadOnly;
		},
		get uploadDisabled() {
			return !inFolder || isReadOnly;
		},
		/** Whether items can be created in a folder, which need not be the open one. */
		isWritable(path: string) {
			const mount = mountOf(path);
			return mount !== null && !mount.readOnly;
		},

		/** Warms the first page of a folder the user is about to open. Skips folders loaded moments ago. */
		prefetchDirectory(path: string) {
			void queryClient.prefetchInfiniteQuery({
				...directoryQueryOptions(path, directoryOptions),
				staleTime: CONFIG.query.prefetchStaleTimeMs
			});
		},
		/** Marks the folder (and the search) stale so it loads again. */
		refresh() {
			void queryClient.invalidateQueries({ queryKey: fileQueryKeys.directory(location.path) });
			if (location.isSearchActive) {
				void queryClient.invalidateQueries({
					queryKey: fileQueryKeys.search(location.path, location.trimmedSearchQuery)
				});
			}
		},
		/** Marks every loaded folder stale, for changes that may land outside the open one. */
		refreshAll() {
			void queryClient.invalidateQueries({ queryKey: fileQueryKeys.directories() });
			void queryClient.invalidateQueries({ queryKey: fileQueryKeys.searches() });
		},
		/** Mounts and drive space change when Trash is restored or a drive is mounted. */
		refreshMounts() {
			void queryClient.invalidateQueries({ queryKey: fileQueryKeys.roots() });
			void queryClient.invalidateQueries({ queryKey: fileQueryKeys.directories() });
		},
		get pageRevision() {
			return pageRevision;
		},
		/** The toolbar refresh button and F5. */
		reload() {
			if (location.page !== 'files') pageRevision += 1;
			else if (isAtRoot) {
				void rootsQuery.refetch();
				void driveStatsQuery.refetch();
			} else void directoryQuery.refetch();
		},
		refetchDirectory() {
			void directoryQuery.refetch();
		},
		loadMore() {
			if (!hasMoreItems || directoryQuery.isFetchingNextPage) return;
			void directoryQuery.fetchNextPage();
		}
	};
}

export type BrowseData = ReturnType<typeof useBrowseData>;
