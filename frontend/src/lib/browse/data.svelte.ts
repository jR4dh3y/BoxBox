import type { RemoteCatalogSnapshot, RemoteCatalogState } from './catalog';
import { parseRemotePath } from '#lib/api/remotePaths.js';
import { browseRequests } from './requests.svelte';
import { untrack } from 'svelte';
import type { BrowseSelection } from './selection.svelte';
import type { FileInfo } from '#lib/api/files.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { canPreview } from '#lib/utils/fileTypes.js';
import { sortFiles } from './sortFiles';
import type { BrowseLocation } from './location.svelte';

/**
 * What the browser shows for the current location: drives, a folder listing or search results.
 * Call during component setup so request leases follow the component lifecycle.
 */
export function useBrowseData(
	location: BrowseLocation,
	selection: BrowseSelection,
	catalog: () => RemoteCatalogSnapshot | undefined = () => undefined
) {
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
	const localCatalog = $derived(browseRequests.catalog());
	const folder = $derived(browseRequests.folder(location.path));
	const matches = $derived(browseRequests.matches(location.path, location.trimmedSearchQuery));
	let acquiredRevision = $state(0);
	$effect(() => {
		const current = folder;
		if (!location.path || isDeploymentRoot) return;
		return untrack(() => {
			acquiredRevision = current.confirmed;
			return browseRequests.observe(location.path);
		});
	});
	$effect(() => {
		void localCatalog;
		const atRoot = location.path === '';
		return untrack(() => browseRequests.observeCatalog(atRoot));
	});
	$effect(() => {
		void matches;
		const path = location.path;
		const query = location.trimmedSearchQuery;
		if (!path || isDeploymentRoot || !location.isSearchActive) return;
		return untrack(() => browseRequests.observeSearch(path, query));
	});

	const isAtRoot = $derived(location.path === '' || isDeploymentRoot);
	const combinedRoots = $derived([
		...(localCatalog.roots.value?.roots ?? []),
		...(catalog()?.roots ?? [])
	]);
	const roots = $derived(
		combinedRoots.filter(
			(root, index) => combinedRoots.findLastIndex((item) => item.name === root.name) === index
		)
	);
	const combinedDrives = $derived(
		isDeploymentRoot
			? (catalog()?.drives ?? []).filter((drive) => drive.name.startsWith(location.path + '/'))
			: [...(localCatalog.stats.value?.drives ?? []), ...(catalog()?.drives ?? [])]
	);
	const drives = $derived(
		combinedDrives.filter(
			(drive, index) => combinedDrives.findIndex((item) => item.name === drive.name) === index
		)
	);
	const directoryItems = $derived(folder.value?.items ?? []);
	const searchResults = $derived(matches.value?.results ?? []);

	const displayItems = $derived.by(() => {
		if (source.kind === 'remote' && (location.isSearchActive ? matches.error : folder.error))
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
		if (folder.pending || !folder.value || folder.error || folder.confirmed <= acquiredRevision)
			return;
		const timestamp = folder.confirmed;
		const items = directoryItems;
		const path = folder.value.path;
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
		if (isAtRoot)
			return localCatalog.roots.error?.message ?? localCatalog.stats.error?.message ?? null;
		if (location.isSearchActive) return matches.error?.message ?? null;
		return folder.error?.message ?? null;
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
				: localCatalog.roots.pending || localCatalog.stats.pending;
		},
		get isSearching() {
			return location.isSearchActive && matches.pending;
		},
		get isFileListLoading() {
			return location.isSearchActive
				? matches.pending
				: folder.pending && directoryItems.length === 0;
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

		/** Warms a folder the user is about to open without another cache owner. */
		prefetchDirectory(path: string) {
			void browseRequests.prefetch(path);
		},
		refresh() {
			void browseRequests.changed(location.path);
		},
		refreshAll() {
			void browseRequests.changed();
		},
		reload() {
			void (isAtRoot ? browseRequests.changed() : browseRequests.changed(location.path));
		},
		refetchDirectory() {
			void browseRequests.changed(location.path);
		}
	};
}

export type BrowseData = ReturnType<typeof useBrowseData>;
