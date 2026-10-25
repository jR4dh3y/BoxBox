import { untrack } from 'svelte';
import type { BrowseSelection } from './selection.svelte';
import { browseRequests } from './requests.svelte';
import { type MountPoint, type DriveStats, type FileInfo } from '#lib/api/files.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { canPreview } from '#lib/utils/fileTypes.js';
import { isNativePath } from '#lib/api/desktop.js';
import { toastStore } from '#lib/stores/toast.svelte.js';
import { sortFiles } from './sortFiles';
import type { BrowseLocation } from './location.svelte';

/**
 * What the browser shows for the current location: drives, a folder or search results.
 * Call during component setup so pane leases follow the component lifecycle.
 */
export function useBrowseData(
	location: BrowseLocation,
	selection: BrowseSelection,
	revealHidden: () => boolean = () => false
) {
	const settings = $derived(settingsStore.current);
	const includeHidden = $derived(settings.showHiddenFiles || revealHidden());

	const catalog = $derived(browseRequests.catalog());
	const folder = $derived(browseRequests.folder(location.path));
	const matches = $derived(browseRequests.matches(location.path, location.trimmedSearchQuery));
	let acquiredRevision = $state(0);
	$effect(() => {
		const current = folder;
		if (!inFolder) return;
		return untrack(() => {
			acquiredRevision = current.confirmed;
			return browseRequests.observe(location.path);
		});
	});
	$effect(() => {
		void catalog;
		const atRoot = isAtRoot;
		return untrack(() => browseRequests.observeCatalog(atRoot));
	});
	$effect(() => {
		void matches;
		const path = location.path;
		const query = location.trimmedSearchQuery;
		if (!path || !location.isSearchActive) return;
		return untrack(() => browseRequests.observeSearch(path, query));
	});

	const isAtRoot = $derived(location.page === 'files' && location.path === '');
	const inFolder = $derived(location.page === 'files' && location.path !== '');
	// Pages load their own data; reloading one remounts it.
	let pageRevision = $state(0);
	let remoteRoots = $state.raw<MountPoint[]>([]);
	let remoteStats = $state.raw<DriveStats[]>([]);
	const roots = $derived([...(catalog.roots.value?.roots ?? []), ...remoteRoots]);
	const drives = $derived([...(catalog.stats.value?.drives ?? []), ...remoteStats]);
	const directoryItems = $derived(folder.value?.items ?? []);
	const searchResults = $derived(matches.value?.results ?? []);

	const displayItems = $derived.by(() => {
		const items: FileInfo[] = location.isSearchActive ? searchResults : directoryItems;
		const visible = includeHidden ? items : items.filter((item) => !item.name.startsWith('.'));
		return location.isSearchActive
			? visible
			: sortFiles(visible, location.sortBy, location.sortDir);
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

	const fileListError = $derived.by(() => {
		if (isAtRoot) return catalog.roots.error?.message ?? catalog.stats.error?.message ?? null;
		if (location.isSearchActive) return matches.error?.message ?? null;
		return folder.error?.message ?? null;
	});

	function mountOf(path: string) {
		if (!path) return null;
		return roots.find((root) => path === root.name || path.startsWith(`${root.name}/`)) ?? null;
	}
	const isReadOnly = $derived(mountOf(location.path)?.readOnly ?? true);
	$effect(() => {
		const path = location.path;
		if (!isNativePath(path) || !path) return;
		let active = true;
		let stop: (() => void) | undefined;
		void import('./desktopWatch')
			.then(({ subscribeDesktopDirectory }) => {
				if (active) stop = subscribeDesktopDirectory(path);
			})
			.catch(() => {
				if (active) toastStore.error('Folder watching could not load. Reload to retry.');
			});
		return () => {
			active = false;
			stop?.();
		};
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
			return (
				(catalog.roots.pending && !catalog.roots.value) ||
				(isAtRoot && catalog.stats.pending && !catalog.stats.value)
			);
		},
		get isSearching() {
			return location.isSearchActive && matches.pending;
		},
		get isFileListLoading() {
			return location.isSearchActive
				? matches.pending
				: folder.pending && directoryItems.length === 0;
		},
		/** True while the folder is being fetched, including a cached one the server has not confirmed. */
		get isFetchingDirectory() {
			return !location.isSearchActive && folder.pending;
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
			browseRequests.prefetch(path);
		},
		refresh() {
			void browseRequests.changed(location.path);
		},
		refreshAll() {
			void browseRequests.changed();
		},
		refreshMounts() {
			void browseRequests.changed();
		},
		get pageRevision() {
			return pageRevision;
		},
		/** The toolbar refresh button and F5. */
		reload() {
			if (location.page !== 'files') pageRevision += 1;
			else if (isAtRoot) {
				void browseRequests.changed();
			} else if (location.isSearchActive) void matches.invalidate();
			else void browseRequests.changed(location.path);
		},
		refetchDirectory() {
			void browseRequests.changed(location.path);
		}
	};
}

export type BrowseData = ReturnType<typeof useBrowseData>;
