import type { RemoteCatalogSnapshot, RemoteCatalogState } from './catalog';
import { parseRemotePath } from '#lib/api/remotePaths.js';
import { browseRequests } from './requests.svelte';
import { untrack } from 'svelte';
import type { BrowseSelection } from './selection.svelte';
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
import { directoryQueryOptions } from './directoryQuery';
import { sortFiles } from './sortFiles';
import type { BrowseLocation } from './location.svelte';

/**
 * What the browser shows for the current location: drives, a folder listing or search results.
 * Call during component setup because the queries need the component context.
 */
export function useBrowseData(
	location: BrowseLocation,
	selection: BrowseSelection,
	catalog: () => RemoteCatalogSnapshot | undefined = () => undefined
) {
	const queryClient = useQueryClient();
	const settings = $derived(settingsStore.current);
	const source = $derived(parseRemotePath(location.path));
	const isDeploymentRoot = $derived(source.kind === 'remote' && source.path === '');
	const deploymentState = $derived.by((): RemoteCatalogState | null => {
		if (source.kind !== 'remote') return null;
		const snapshot = catalog();
		if (!snapshot || snapshot.status.kind !== 'ready')
			return snapshot?.status ?? { kind: 'loading' };
		return (
			snapshot.states[source.serverId] ?? {
				kind: 'error',
				message: 'This deployment is no longer saved. Open Connections to add it again.'
			}
		);
	});
	const serverName = $derived(
		source.kind === 'remote'
			? catalog()?.servers.find((server) => server.id === source.serverId)?.name
			: undefined
	);
	$effect(() =>
		browseRequests.onChange(() => {
			void queryClient.invalidateQueries({ queryKey: fileQueryKeys.all });
		})
	);

	const rootsQuery = createQuery<RootsResponse>(() => ({
		queryKey: fileQueryKeys.roots(),
		queryFn: () => listRoots()
	}));

	const driveStatsQuery = createQuery<DriveStatsResponse>(() => ({
		queryKey: [...fileQueryKeys.roots(), 'stats'],
		queryFn: () => getDriveStats(),
		enabled: location.path === '' || isDeploymentRoot
	}));

	const directoryQuery = createQuery(() => ({
		...directoryQueryOptions(location.path),
		refetchInterval: source.kind === 'remote' ? 15_000 : false,
		enabled: location.path !== '' && !isDeploymentRoot
	}));
	const searchQueryResult = createQuery<SearchResponse>(() => ({
		queryKey: fileQueryKeys.search(location.path, location.trimmedSearchQuery),
		queryFn: ({ signal }) => search(location.path, location.trimmedSearchQuery, signal),
		enabled: location.path !== '' && !isDeploymentRoot && location.isSearchActive
	}));

	const isAtRoot = $derived(location.path === '' || isDeploymentRoot);
	const combinedRoots = $derived([...(rootsQuery.data?.roots ?? []), ...(catalog()?.roots ?? [])]);
	const roots = $derived(
		combinedRoots.filter(
			(root, index) => combinedRoots.findLastIndex((item) => item.name === root.name) === index
		)
	);
	const combinedDrives = $derived(
		isDeploymentRoot
			? (catalog()?.drives ?? []).filter((drive) => drive.name.startsWith(location.path + '/'))
			: [...(driveStatsQuery.data?.drives ?? []), ...(catalog()?.drives ?? [])]
	);
	const drives = $derived(
		combinedDrives.filter(
			(drive, index) => combinedDrives.findIndex((item) => item.name === drive.name) === index
		)
	);
	const directoryItems = $derived(directoryQuery.data?.items ?? []);
	const searchResults = $derived(searchQueryResult.data?.results ?? []);

	const displayItems = $derived.by(() => {
		if (
			source.kind === 'remote' &&
			(location.isSearchActive ? searchQueryResult.isError : directoryQuery.isError)
		)
			return [];
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

	const currentMount = $derived(
		location.path
			? [...roots]
					.sort((a, b) => b.name.length - a.name.length)
					.find((root) => location.path === root.name || location.path.startsWith(`${root.name}/`))
			: null
	);
	const isReadOnly = $derived(
		isDeploymentRoot ||
			(source.kind === 'remote' && !currentMount) ||
			(currentMount?.readOnly ?? false)
	);
	const fileListError = $derived.by(() => {
		if (isAtRoot) return rootsQuery.error?.message ?? driveStatsQuery.error?.message ?? null;
		if (location.isSearchActive) return searchQueryResult.error?.message ?? null;
		return directoryQuery.error?.message ?? null;
	});

	return {
		get isDeploymentRoot() {
			return isDeploymentRoot;
		},
		get serverName() {
			return serverName;
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
		get deploymentError() {
			return deploymentState?.kind === 'error' ? deploymentState.message : null;
		},
		get isLoadingDrives() {
			return isDeploymentRoot
				? deploymentState?.kind === 'loading'
				: rootsQuery.isLoading || driveStatsQuery.isLoading;
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
			if (isAtRoot) {
				void browseRequests.changed();
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
