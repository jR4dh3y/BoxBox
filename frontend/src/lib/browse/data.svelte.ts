import { untrack } from 'svelte';
import type { BrowseSelection } from './selection.svelte';
import { createQuery, useQueryClient } from '@tanstack/svelte-query';
import {
	getDriveStats,
	listRoots,
	search,
	type DriveStatsResponse,
	type MountPoint,
	type DriveStats,
	type FileInfo,
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
import { sortFiles } from './sortFiles';
import type { BrowseLocation } from './location.svelte';

/**
 * What the browser shows for the current location: drives, a folder listing or search results.
 * Call during component setup because the queries need the component context.
 */
export function useBrowseData(location: BrowseLocation, selection: BrowseSelection) {
	const queryClient = useQueryClient();
	const settings = $derived(settingsStore.current);
	const includeHidden = $derived(settings.showHiddenFiles || revealHidden());

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

	const directoryQuery = createQuery(() => ({
		...directoryQueryOptions(location.path),
		enabled: location.path !== ''
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
	let remoteRoots = $state.raw<MountPoint[]>([]);
	let remoteStats = $state.raw<DriveStats[]>([]);
	const roots = $derived([...(rootsQuery.data?.roots ?? []), ...remoteRoots]);
	const drives = $derived([...(driveStatsQuery.data?.drives ?? []), ...remoteStats]);
	const directoryItems = $derived(directoryQuery.data?.items ?? []);
	const searchResults = $derived(searchQueryResult.data?.results ?? []);

	const displayItems = $derived.by(() => {
		const items: FileInfo[] = location.isSearchActive ? searchResults : directoryItems;
		const visible = settings.showHiddenFiles
			? items
			: items.filter((item) => !item.name.startsWith('.'));
		return sortFiles(visible, location.sortBy, location.sortDir);
	});

	let confirmed: { path: string; timestamp: number; items: readonly FileInfo[] } | undefined;
	$effect(() => {
		// Cached data and failed refreshes cannot establish that a selected file vanished.
		if (
			directoryQuery.isFetching ||
			!directoryQuery.isSuccess ||
			!directoryQuery.isFetchedAfterMount
		)
			return;
		const timestamp = directoryQuery.dataUpdatedAt;
		const items = directoryItems;
		const path = directoryQuery.data.path;
		untrack(() => {
			if (
				confirmed?.path === path &&
				confirmed.timestamp === timestamp &&
				confirmed.items === items
			)
				return;
			confirmed = { path, timestamp, items };
			if (!location.isSearchActive) selection.retain(items, path);
		});
	});

	let confirmed: { path: string; timestamp: number; items: readonly FileInfo[] } | undefined;
	$effect(() => {
		// Cached data and failed refreshes cannot establish that a selected file vanished.
		if (
			directoryQuery.isFetching ||
			!directoryQuery.isSuccess ||
			!directoryQuery.isFetchedAfterMount
		)
			return;
		const timestamp = directoryQuery.dataUpdatedAt;
		const items = directoryItems;
		const path = directoryQuery.data.path;
		untrack(() => {
			if (
				confirmed?.path === path &&
				confirmed.timestamp === timestamp &&
				confirmed.items === items
			)
				return;
			confirmed = { path, timestamp, items };
			if (!location.isSearchActive) selection.retain(items, path);
		});
	});

	const currentMount = $derived(
		location.path
			? roots.find(
					(root) => location.path === root.name || location.path.startsWith(`${root.name}/`)
				)
			: null
	);
	const isReadOnly = $derived(currentMount?.readOnly ?? false);
	const fileListError = $derived.by(() => {
		if (isAtRoot) return rootsQuery.error?.message ?? driveStatsQuery.error?.message ?? null;
		if (location.isSearchActive) return searchQueryResult.error?.message ?? null;
		return directoryQuery.error?.message ?? null;
	});

	return {
		setRemoteCatalog(catalog: { roots: MountPoint[]; drives: DriveStats[] }) {
			remoteRoots = catalog.roots;
			remoteStats = catalog.drives;
		},
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
		get fileListError() {
			return fileListError;
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
			if (isAtRoot) {
				void rootsQuery.refetch();
				void driveStatsQuery.refetch();
			} else if (location.isSearchActive) void searchQueryResult.refetch();
			else void directoryQuery.refetch();
		},
		refetchDirectory() {
			void directoryQuery.refetch();
		}
	};
}

export type BrowseData = ReturnType<typeof useBrowseData>;
