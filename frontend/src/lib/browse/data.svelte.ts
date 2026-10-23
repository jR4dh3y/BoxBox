import { createQuery, useQueryClient } from '@tanstack/svelte-query';
import {
	getDriveStats,
	listRoots,
	search,
	type DriveStatsResponse,
	type FileInfo,
	type RootsResponse,
	type SearchResponse
} from '#lib/api/files.js';
import { CONFIG } from '#lib/config.js';
import { fileQueryKeys } from '#lib/stores/files.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { canPreview } from '#lib/utils/fileTypes.js';
import { isLocalDesktop } from '#lib/api/desktop.js';
import { coalesceRefresh, watchDesktopDirectory } from './desktopWatch';
import { directoryQueryOptions } from './directoryQuery';
import { sortFiles } from './sortFiles';
import type { BrowseLocation } from './location.svelte';

/**
 * What the browser shows for the current location: drives, a folder or search results.
 * Call during component setup because the queries need the component context.
 */
export function useBrowseData(location: BrowseLocation, revealHidden: () => boolean = () => false) {
	const queryClient = useQueryClient();
	const settings = $derived(settingsStore.current);
	const includeHidden = $derived(settings.showHiddenFiles || revealHidden());

	const rootsQuery = createQuery<RootsResponse>(() => ({
		queryKey: fileQueryKeys.roots(),
		queryFn: () => listRoots()
	}));

	const driveStatsQuery = createQuery<DriveStatsResponse>(() => ({
		queryKey: [...fileQueryKeys.roots(), 'stats'],
		queryFn: () => getDriveStats(),
		enabled: location.path === ''
	}));

	const directoryQuery = createQuery(() => ({
		...directoryQueryOptions(location.path),
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
	const directoryItems = $derived(directoryQuery.data?.items ?? []);
	const searchResults = $derived(searchQueryResult.data?.results ?? []);

	const displayItems = $derived.by(() => {
		const items: FileInfo[] = location.isSearchActive ? searchResults : directoryItems;
		const visible = includeHidden ? items : items.filter((item) => !item.name.startsWith('.'));
		return sortFiles(visible, location.sortBy, location.sortDir);
	});

	const currentMount = $derived(
		location.path
			? roots.find(
					(root) => location.path === root.name || location.path.startsWith(`${root.name}/`)
				)
			: null
	);
	const isReadOnly = $derived(currentMount?.readOnly ?? false);
	$effect(() => {
		const path = location.path;
		if (!isLocalDesktop() || !path) return;
		// A refetch is the whole folder; let each one finish instead of cancelling it on every change.
		const refresh = coalesceRefresh(() =>
			Promise.all([
				queryClient.invalidateQueries(
					{ queryKey: fileQueryKeys.directory(path) },
					{ cancelRefetch: false }
				),
				queryClient.invalidateQueries(
					{ queryKey: fileQueryKeys.searches() },
					{ cancelRefetch: false }
				)
			])
		);
		return watchDesktopDirectory(path, refresh);
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
		/** True while the folder is being fetched, including a cached one the server has not confirmed. */
		get isFetchingDirectory() {
			return !location.isSearchActive && directoryQuery.isFetching;
		},
		get displayItems() {
			return displayItems;
		},
		get previewableFiles() {
			return displayItems.filter((item) => !item.isDir && canPreview(item.name));
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

		/** Warms a folder the user is about to open. Skips folders loaded moments ago. */
		prefetchDirectory(path: string) {
			void queryClient.prefetchQuery({
				...directoryQueryOptions(path),
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
		/** The toolbar refresh button. */
		reload() {
			if (isAtRoot) void driveStatsQuery.refetch();
			else void directoryQuery.refetch();
		},
		refetchDirectory() {
			void directoryQuery.refetch();
		}
	};
}

export type BrowseData = ReturnType<typeof useBrowseData>;
