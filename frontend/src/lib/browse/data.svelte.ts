import {
	createInfiniteQuery,
	createQuery,
	useQueryClient,
	type InfiniteData
} from '@tanstack/svelte-query';
import {
	getDriveStats,
	listDirectory,
	listRoots,
	search,
	type DriveStatsResponse,
	type FileInfo,
	type FileList as FileListType,
	type ListOptions,
	type RootsResponse,
	type SearchResponse
} from '$lib/api/files';
import { fileQueryKeys } from '$lib/stores/files';
import { settingsStore } from '$lib/stores/settings.svelte';
import { canPreview } from '$lib/utils/fileTypes';
import type { BrowseLocation } from './location.svelte';

/**
 * What the browser shows for the current location: drives, a folder page or search results.
 * Call during component setup because the queries need the component context.
 */
export function useBrowseData(location: BrowseLocation) {
	const queryClient = useQueryClient();
	const settings = $derived(settingsStore.current);

	const directoryOptions = $derived<Omit<ListOptions, 'page'>>({
		pageSize: 50,
		sortBy: location.sortBy,
		sortDir: location.sortDir,
		includeHidden: settings.showHiddenFiles
	});

	const rootsQuery = createQuery<RootsResponse>(() => ({
		queryKey: fileQueryKeys.roots(),
		queryFn: () => listRoots()
	}));

	const driveStatsQuery = createQuery<DriveStatsResponse>(() => ({
		queryKey: [...fileQueryKeys.roots(), 'stats'],
		queryFn: () => getDriveStats(),
		enabled: location.path === ''
	}));

	const directoryQuery = createInfiniteQuery<
		FileListType,
		Error,
		InfiniteData<FileListType>,
		ReturnType<typeof fileQueryKeys.list>,
		number
	>(() => ({
		queryKey: fileQueryKeys.list(location.path, directoryOptions),
		queryFn: ({ pageParam, signal }) =>
			listDirectory(location.path, { ...directoryOptions, page: pageParam }, signal),
		initialPageParam: 1,
		getNextPageParam: (lastPage) =>
			lastPage.page * lastPage.pageSize < lastPage.totalCount ? lastPage.page + 1 : undefined,
		enabled: location.path !== ''
	}));

	const searchQueryResult = createQuery<SearchResponse>(() => ({
		queryKey: fileQueryKeys.search(location.path, location.trimmedSearchQuery),
		queryFn: ({ signal }) => search(location.path, location.trimmedSearchQuery, signal),
		enabled: location.path !== '' && location.isSearchActive
	}));

	const isAtRoot = $derived(location.path === '');
	const roots = $derived(rootsQuery.data?.roots ?? []);
	const drives = $derived(driveStatsQuery.data?.drives ?? []);
	const directoryPages = $derived(directoryQuery.data?.pages ?? []);
	const directoryItems = $derived(directoryPages.flatMap((directoryPage) => directoryPage.items));
	const searchResults = $derived(searchQueryResult.data?.results ?? []);

	const displayItems = $derived.by(() => {
		const items: FileInfo[] = location.isSearchActive ? searchResults : directoryItems;
		return settings.showHiddenFiles ? items : items.filter((item) => !item.name.startsWith('.'));
	});

	const currentMount = $derived(
		location.path
			? roots.find(
					(root) => location.path === root.name || location.path.startsWith(`${root.name}/`)
				)
			: null
	);
	const isReadOnly = $derived(currentMount?.readOnly ?? false);
	const hasMoreItems = $derived(
		!location.isSearchActive && !isAtRoot && directoryQuery.hasNextPage
	);

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
			return !isAtRoot && !isReadOnly;
		},
		get uploadDisabled() {
			return isAtRoot || isReadOnly;
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
		/** The toolbar refresh button. */
		reload() {
			if (isAtRoot) void driveStatsQuery.refetch();
			else void directoryQuery.refetch();
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
