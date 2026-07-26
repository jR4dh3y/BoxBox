<script lang="ts">
	/**
	 * Browse page - main file browser interface (FilePilot style)
	 */
	import {
		createInfiniteQuery,
		createQuery,
		useQueryClient,
		type InfiniteData
	} from '@tanstack/svelte-query';
	import { afterNavigate, goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import Toolbar from '$lib/components/Toolbar.svelte';
	import FileList from '$lib/components/FileList.svelte';
	import FileGrid from '$lib/components/FileGrid.svelte';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import DriveCard from '$lib/components/DriveCard.svelte';
	import FilePreview from '$lib/components/FilePreview.svelte';
	import BrowseDialogs from '$lib/components/BrowseDialogs.svelte';
	import UploadPanel from '$lib/components/UploadPanel.svelte';
	import Toast from '$lib/components/ui/Toast.svelte';
	import { Spinner } from '$lib/components/ui';
	import { fileQueryKeys } from '$lib/stores/files';
	import { settingsStore } from '$lib/stores/settings';
	import { clipboardStore } from '$lib/stores/clipboard.svelte';
	import { uploadStore } from '$lib/stores/upload.svelte';
	import { toastStore } from '$lib/stores/toast.svelte';
	import { jobsStore } from '$lib/stores/jobs';
	import {
		listRoots,
		getDriveStats,
		listDirectory,
		search,
		createDirectory,
		createFile,
		rename,
		deleteFile,
		getDownloadUrl
	} from '$lib/api/files';
	import { createCopyJob, createMoveJob, createDeleteJob } from '$lib/api/jobs';
	import { canPreview, getFileTypeDescription } from '$lib/utils/fileTypes';
	import type { SortField, SortDir, ViewMode } from '$lib/types/files';
	import type {
		FileInfo,
		FileList as FileListType,
		ListOptions,
		DriveStatsResponse,
		RootsResponse,
		SearchResponse
	} from '$lib/api/files';
	import { SvelteSet } from 'svelte/reactivity';

	let selectedPaths = $state(new Set<string>());
	let previewFile = $state<FileInfo | null>(null);
	type BrowsePageState = App.PageState & { browseHistoryIndex?: number };
	let historyMaxIndex = $state((page.state as BrowsePageState).browseHistoryIndex ?? 0);

	// Upload state
	let fileInputEl: HTMLInputElement;
	let isDragOver = $state(false);

	// Rename dialog state
	let renameDialog = $state<{ open: boolean; file: FileInfo | null; newName: string }>({
		open: false,
		file: null,
		newName: ''
	});

	// Delete confirmation dialog state
	let deleteDialog = $state<{ open: boolean; items: FileInfo[] }>({
		open: false,
		items: []
	});

	// Create file/folder dialog state
	let createDialog = $state<{ open: boolean; type: 'file' | 'directory'; name: string }>({
		open: false,
		type: 'file',
		name: ''
	});

	// Properties dialog state
	let propertiesDialog = $state<{ open: boolean; file: FileInfo | null }>({
		open: false,
		file: null
	});

	const settings = $derived($settingsStore);
	const path = $derived(page.url.searchParams.get('path')?.replace(/^\/+|\/+$/g, '') ?? '');
	const segments = $derived(path ? path.split('/') : []);
	const searchQuery = $derived(page.url.searchParams.get('q') ?? '');
	const trimmedSearchQuery = $derived(searchQuery.trim());
	const isSearchActive = $derived(trimmedSearchQuery.length >= 2);
	const sortBy = $derived<SortField>(parseSortField(page.url.searchParams.get('sort')));
	const sortDir = $derived<SortDir>(
		page.url.searchParams.get('dir') === 'desc'
			? 'desc'
			: page.url.searchParams.get('dir') === 'asc'
				? 'asc'
				: settings.defaultSortDir
	);
	const viewMode = $derived<ViewMode>(
		page.url.searchParams.get('view') === 'grid'
			? 'grid'
			: page.url.searchParams.get('view') === 'list'
				? 'list'
				: settings.defaultViewMode
	);
	const directoryOptions = $derived<Omit<ListOptions, 'page'>>({
		pageSize: 50,
		sortBy,
		sortDir,
		includeHidden: settings.showHiddenFiles
	});
	const queryClient = useQueryClient();

	function parseSortField(value: string | null): SortField {
		if (value === 'size' || value === 'modTime' || value === 'type') return value;
		if (value === 'name') return value;
		return settingsStore.getSetting('defaultSortBy');
	}

	const rootsQuery = createQuery<RootsResponse>(() => ({
		queryKey: fileQueryKeys.roots(),
		queryFn: () => listRoots()
	}));

	const driveStatsQuery = createQuery<DriveStatsResponse>(() => ({
		queryKey: [...fileQueryKeys.roots(), 'stats'],
		queryFn: () => getDriveStats(),
		enabled: path === ''
	}));

	const directoryQuery = createInfiniteQuery<
		FileListType,
		Error,
		InfiniteData<FileListType>,
		ReturnType<typeof fileQueryKeys.list>,
		number
	>(() => ({
		queryKey: fileQueryKeys.list(path, directoryOptions),
		queryFn: ({ pageParam, signal }) =>
			listDirectory(path, { ...directoryOptions, page: pageParam }, signal),
		initialPageParam: 1,
		getNextPageParam: (lastPage) =>
			lastPage.page * lastPage.pageSize < lastPage.totalCount ? lastPage.page + 1 : undefined,
		enabled: path !== ''
	}));

	const searchQueryResult = createQuery<SearchResponse>(() => ({
		queryKey: fileQueryKeys.search(path, trimmedSearchQuery),
		queryFn: ({ signal }) => search(path, trimmedSearchQuery, signal),
		enabled: path !== '' && isSearchActive
	}));

	const isLoading = $derived(directoryQuery.isLoading);
	const isLoadingMore = $derived(!isSearchActive && directoryQuery.isFetchingNextPage);
	const directoryPages = $derived(directoryQuery.data?.pages ?? []);
	const directoryItems = $derived(directoryPages.flatMap((directoryPage) => directoryPage.items));
	const directoryTotalCount = $derived(directoryPages[0]?.totalCount ?? 0);
	const isFileListLoading = $derived(
		isSearchActive ? searchQueryResult.isFetching : isLoading && directoryItems.length === 0
	);
	const searchResults = $derived(searchQueryResult.data?.results ?? []);
	const roots = $derived(rootsQuery.data?.roots ?? []);
	const drives = $derived(driveStatsQuery.data?.drives ?? []);
	const isAtRoot = $derived(path === '');

	// Clipboard state for context menu
	const canPaste = $derived(clipboardStore.hasItems);
	const favoritePaths = $derived(
		new SvelteSet(settings.favoriteFolders.map((folder) => folder.path))
	);
	const cutPaths = $derived.by(() => {
		if (clipboardStore.operation === 'cut') {
			return new SvelteSet(clipboardStore.items.map((i) => i.path));
		}
		return new SvelteSet<string>();
	});

	const displayItems = $derived.by(() => {
		let items: FileInfo[];

		if (isSearchActive) {
			items = searchResults;
		} else {
			items = directoryItems;
		}

		if (!settings.showHiddenFiles) {
			items = items.filter((item) => !item.name.startsWith('.'));
		}

		return items;
	});
	const previewableFiles = $derived(
		displayItems.filter((item) => !item.isDir && canPreview(item.name))
	);
	const hasMoreItems = $derived(!isSearchActive && !isAtRoot && directoryQuery.hasNextPage);
	const statusTotalCount = $derived.by(() => {
		if (isAtRoot || isSearchActive) return undefined;
		return directoryTotalCount;
	});
	const emptyListMessage = $derived.by(() => {
		if (isSearchActive) {
			return `No matches for "${trimmedSearchQuery}" in this folder`;
		}
		return 'This folder is empty';
	});

	const itemCount = $derived(isAtRoot ? drives.length : displayItems.length);
	const selectedCount = $derived(selectedPaths.size);
	const historyIndex = $derived((page.state as BrowsePageState).browseHistoryIndex ?? 0);
	const canGoBack = $derived(historyIndex > 0);
	const canGoForward = $derived(historyIndex < historyMaxIndex);
	const canGoUp = $derived(segments.length > 0);
	const currentMount = $derived(
		path ? roots.find((root) => path === root.name || path.startsWith(`${root.name}/`)) : null
	);
	const isCurrentLocationReadOnly = $derived(currentMount?.readOnly ?? false);
	const canCreate = $derived(!isAtRoot && !isCurrentLocationReadOnly);

	afterNavigate((navigation) => {
		if (navigation.type !== 'popstate') return;
		selectedPaths = new Set();
	});

	function getErrorMessage(error: unknown, fallback: string): string {
		return error instanceof Error ? error.message : fallback;
	}

	function refreshCurrentLocation() {
		void queryClient.invalidateQueries({ queryKey: fileQueryKeys.directory(path) });
		if (isSearchActive) {
			void queryClient.invalidateQueries({
				queryKey: fileQueryKeys.search(path, trimmedSearchQuery)
			});
		}
	}

	function updateBrowseParams(
		updates: Record<string, string | null>,
		options: { replaceState?: boolean } = {}
	) {
		const url = new URL(page.url);
		for (const [key, value] of Object.entries(updates)) {
			if (value) url.searchParams.set(key, value);
			else url.searchParams.delete(key);
		}
		if (options.replaceState) {
			void goto(`${resolve('/browse')}${url.search}${url.hash}`, {
				replaceState: true,
				noScroll: true,
				keepFocus: true,
				state: page.state
			});
			return;
		}

		const nextIndex = historyIndex + 1;
		historyMaxIndex = nextIndex;
		void goto(`${resolve('/browse')}${url.search}${url.hash}`, {
			noScroll: true,
			keepFocus: true,
			state: { ...page.state, browseHistoryIndex: nextIndex }
		});
	}

	function handleNavigate(newPath: string) {
		selectedPaths = new Set();
		updateBrowseParams({ path: newPath || null, q: null });
	}

	function handleBack() {
		history.back();
	}

	function handleForward() {
		history.forward();
	}

	function handleUp() {
		if (canGoUp) {
			const parentPath = segments.slice(0, -1).join('/');
			handleNavigate(parentPath);
		}
	}

	function handleRefresh() {
		if (isAtRoot) {
			driveStatsQuery.refetch();
		} else {
			directoryQuery.refetch();
		}
	}

	function handleSettings() {
		goto(resolve('/settings'));
	}

	function handleFileClick(file: FileInfo) {
		if (file.isDir) {
			handleNavigate(file.path);
		} else {
			if (!canPreview(file.name)) {
				toastStore.info(`Preview not available for ${getFileTypeDescription(file.name)}`);
				return;
			}

			previewFile = file;
		}
	}

	function handleClosePreview() {
		previewFile = null;
	}

	function handlePreviewNavigate(file: FileInfo) {
		previewFile = file;
	}

	function handleSearchInput(query: string) {
		updateBrowseParams({ q: query.trim() || null }, { replaceState: true });
	}

	function handleSearchClear() {
		updateBrowseParams({ q: null }, { replaceState: true });
	}

	function handleSortChange(field: SortField, dir: SortDir) {
		updateBrowseParams({ sort: field, dir }, { replaceState: true });
	}

	function handleSelectionChange(paths: Set<string>) {
		selectedPaths = paths;
	}

	function handleViewModeChange(mode: ViewMode) {
		updateBrowseParams({ view: mode }, { replaceState: true });
	}

	function handleLoadMore() {
		if (!hasMoreItems || directoryQuery.isFetchingNextPage) return;
		void directoryQuery.fetchNextPage();
	}

	/**
	 * Handle context menu actions
	 */
	async function handleContextMenuAction(action: string, items: FileInfo[]) {
		switch (action) {
			case 'new-file':
				openCreateDialog('file');
				break;

			case 'new-folder':
				openCreateDialog('directory');
				break;

			case 'copy':
				clipboardStore.copy(items);
				break;

			case 'cut':
				clipboardStore.cut(items);
				break;

			case 'paste':
				await handlePaste();
				break;

			case 'pin':
				if (items.length === 1 && items[0].isDir) {
					settingsStore.pinFavoriteFolder({ name: items[0].name, path: items[0].path });
					toastStore.success(`${items[0].name} pinned to favorites`);
				}
				break;

			case 'unpin':
				if (items.length === 1 && items[0].isDir) {
					settingsStore.unpinFavoriteFolder(items[0].path);
					toastStore.success(`${items[0].name} unpinned from favorites`);
				}
				break;

			case 'rename':
				if (items.length === 1) {
					renameDialog = {
						open: true,
						file: items[0],
						newName: items[0].name
					};
				}
				break;

			case 'delete':
				if (settings.confirmDelete) {
					deleteDialog = {
						open: true,
						items: items
					};
				} else {
					await deleteItems(items);
				}
				break;

			case 'download':
				handleDownload(items);
				break;

			case 'properties':
				if (items.length === 1) {
					propertiesDialog = {
						open: true,
						file: items[0]
					};
				}
				break;
		}
	}

	function openCreateDialog(type: 'file' | 'directory') {
		if (!canCreate) {
			toastStore.error('Cannot create items in this location');
			return;
		}

		createDialog = {
			open: true,
			type,
			name: type === 'file' ? 'untitled.txt' : 'New Folder'
		};
	}

	function closeCreateDialog() {
		createDialog = { open: false, type: 'file', name: '' };
	}

	function validateItemName(value: string): string | null {
		const name = value.trim();
		if (!name) return null;
		if (name.includes('/') || name.includes('\\')) {
			toastStore.error('Name cannot contain path separators');
			return null;
		}
		return name;
	}

	async function handleCreateConfirm() {
		if (!path) return;
		const name = validateItemName(createDialog.name);
		if (!name) return;

		try {
			const created =
				createDialog.type === 'file'
					? await createFile(path, name)
					: await createDirectory(path, name);

			closeCreateDialog();
			selectedPaths = new Set([created.path]);
			toastStore.success(`${created.name} created`);
			refreshCurrentLocation();
		} catch (error) {
			toastStore.error(getErrorMessage(error, 'Create failed'));
		}
	}

	/**
	 * Handle paste operation
	 */
	async function handlePaste() {
		if (!clipboardStore.hasItems || !path) return;

		const operation = clipboardStore.operation;
		const items = clipboardStore.items;

		try {
			for (const item of items) {
				const destPath = `${path}/${item.name}`;
				if (operation === 'copy') {
					jobsStore.upsertJob(await createCopyJob(item.path, destPath));
				} else if (operation === 'cut') {
					jobsStore.upsertJob(await createMoveJob(item.path, destPath));
				}
			}

			// Clear clipboard after cut operation
			if (operation === 'cut') {
				clipboardStore.clear();
			}

			// Refresh directory listing
			directoryQuery.refetch();
		} catch (error) {
			console.error('Paste operation failed:', error);
			toastStore.error(getErrorMessage(error, 'Paste failed'));
		}
	}

	/**
	 * Handle file download
	 */
	function handleDownload(items: FileInfo[]) {
		for (const item of items) {
			if (!item.isDir) {
				const downloadUrl = getDownloadUrl(item.path);
				window.open(downloadUrl, '_blank');
			}
		}
	}

	/**
	 * Handle rename confirmation
	 */
	async function handleRenameConfirm() {
		if (!renameDialog.file) return;
		const newName = validateItemName(renameDialog.newName);
		if (!newName) return;

		const oldPath = renameDialog.file.path;
		const parentPath = oldPath.substring(0, oldPath.lastIndexOf('/'));
		const newPath = parentPath ? `${parentPath}/${newName}` : newName;

		try {
			await rename(oldPath, newPath);
			renameDialog = { open: false, file: null, newName: '' };
			directoryQuery.refetch();
		} catch (error) {
			console.error('Rename failed:', error);
			toastStore.error(getErrorMessage(error, 'Rename failed'));
		}
	}

	/**
	 * Delete files immediately or enqueue directory deletion jobs.
	 */
	async function deleteItems(items: FileInfo[]) {
		if (items.length === 0) return;

		try {
			for (const item of items) {
				if (item.isDir) {
					// Use job for directory deletion
					jobsStore.upsertJob(await createDeleteJob(item.path));
				} else {
					await deleteFile(item.path);
				}
			}
			selectedPaths = new Set();
			directoryQuery.refetch();
		} catch (error) {
			console.error('Delete failed:', error);
			toastStore.error(getErrorMessage(error, 'Delete failed'));
		}
	}

	/**
	 * Handle delete confirmation
	 */
	async function handleDeleteConfirm() {
		await deleteItems(deleteDialog.items);
		deleteDialog = { open: false, items: [] };
	}

	function handleFileSaved(file: FileInfo) {
		previewFile = file;
		refreshCurrentLocation();
	}

	// Setup upload store callbacks
	uploadStore.onComplete = (fileName: string, success: boolean, error?: string) => {
		if (success) {
			toastStore.success(`${fileName} uploaded successfully`);
		} else {
			toastStore.error(`Upload failed: ${error || 'Unknown error'}`);
		}
	};

	let uploadRefreshTimer: ReturnType<typeof setTimeout> | undefined;
	uploadStore.onRefreshNeeded = () => {
		if (uploadRefreshTimer) clearTimeout(uploadRefreshTimer);
		uploadRefreshTimer = setTimeout(refreshCurrentLocation, 250);
	};

	/**
	 * Handle upload button click - open file picker
	 */
	function handleUploadClick() {
		fileInputEl?.click();
	}

	/**
	 * Handle files selected from file picker
	 */
	function handleFileInputChange(event: Event) {
		const input = event.target as HTMLInputElement;
		const files = input.files;
		if (files && files.length > 0) {
			startUploads(Array.from(files));
		}
		// Reset input so the same file can be selected again
		input.value = '';
	}

	/**
	 * Handle drag over event
	 */
	function handleDragOver(event: DragEvent) {
		event.preventDefault();
		event.stopPropagation();
		if (!isAtRoot) {
			isDragOver = true;
		}
	}

	/**
	 * Handle drag leave event
	 */
	function handleDragLeave(event: DragEvent) {
		event.preventDefault();
		event.stopPropagation();
		isDragOver = false;
	}

	/**
	 * Handle drop event
	 */
	function handleDrop(event: DragEvent) {
		event.preventDefault();
		event.stopPropagation();
		isDragOver = false;

		if (isAtRoot) {
			toastStore.warning('Navigate to a folder first to upload files');
			return;
		}

		// Check if current mount is read-only
		if (isCurrentLocationReadOnly) {
			toastStore.error('Cannot upload to read-only location');
			return;
		}

		const files = event.dataTransfer?.files;
		if (files && files.length > 0) {
			startUploads(Array.from(files));
		}
	}

	/**
	 * Start uploading files
	 */
	function startUploads(files: File[]) {
		if (!path) {
			toastStore.warning('Navigate to a folder first to upload files');
			return;
		}

		// Check if current mount is read-only
		if (isCurrentLocationReadOnly) {
			toastStore.error('Cannot upload to read-only location');
			return;
		}

		uploadStore.addFiles(files, path);
	}

	// Derived: is upload disabled (at root or read-only)
	const uploadDisabled = $derived.by(() => {
		if (isAtRoot) return true;
		return isCurrentLocationReadOnly;
	});
</script>

<svelte:head>
	<title>BoxBox</title>
</svelte:head>

<div class="flex h-screen w-full overflow-hidden bg-surface-primary">
	<!-- Sidebar -->
	<Sidebar currentPath={path} {roots} onNavigate={handleNavigate} />

	<!-- Main content area -->
	<div class="flex min-w-0 flex-1 flex-col">
		<!-- Toolbar with navigation and path bar -->
		<Toolbar
			pathSegments={segments}
			{canGoBack}
			{canGoForward}
			{canGoUp}
			onBack={handleBack}
			onForward={handleForward}
			onUp={handleUp}
			onNavigate={handleNavigate}
			onRefresh={handleRefresh}
			onSettings={handleSettings}
			onUpload={handleUploadClick}
			{uploadDisabled}
			showSearch={!isAtRoot}
			searchValue={searchQuery}
			searchLoading={isSearchActive && searchQueryResult.isFetching}
			onSearchInput={handleSearchInput}
			onSearchClear={handleSearchClear}
			includeHiddenSuggestions={settings.showHiddenFiles}
		/>

		<!-- File list or Drive cards -->
		<div
			class="relative flex-1 overflow-auto"
			ondragover={handleDragOver}
			ondragleave={handleDragLeave}
			ondrop={handleDrop}
			role="region"
			aria-label="File browser content"
		>
			<!-- Drag-drop overlay -->
			{#if isDragOver && !isAtRoot}
				<div
					class="pointer-events-none absolute inset-0 z-20 flex items-center justify-center border-2 border-dashed border-accent bg-accent/10"
				>
					<div class="rounded-lg bg-surface-primary/90 px-6 py-4 shadow-lg backdrop-blur-sm">
						<span class="text-lg font-medium text-accent">Drop files to upload here</span>
					</div>
				</div>
			{/if}

			{#if isAtRoot}
				<!-- This Server view - show drive cards -->
				<div class="p-6">
					{#if rootsQuery.isLoading || driveStatsQuery.isLoading}
						<div class="flex items-center gap-2 py-5 text-sm text-text-secondary">
							<Spinner size="sm" />
							<span>Loading drives...</span>
						</div>
					{:else if drives.length === 0}
						<div class="py-5 text-sm text-text-secondary">No configured storage found</div>
					{:else}
						<div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
							{#each drives as drive (drive.name)}
								<DriveCard
									{drive}
									onClick={() => handleNavigate(drive.name)}
								/>
							{/each}
						</div>
					{/if}
				</div>
			{:else if viewMode === 'grid'}
				<FileGrid
					items={displayItems}
					emptyMessage={emptyListMessage}
					{selectedPaths}
					isLoading={isFileListLoading}
					compactMode={settings.compactMode}
					{cutPaths}
					{favoritePaths}
					{canPaste}
					{canCreate}
					showFileExtensions={settings.showFileExtensions}
					previewOnSingleClick={settings.previewOnSingleClick}
					onItemClick={handleFileClick}
					onSelectionChange={handleSelectionChange}
					onContextMenuAction={handleContextMenuAction}
				/>
			{:else}
				<FileList
					items={displayItems}
					{sortBy}
					{sortDir}
					emptyMessage={emptyListMessage}
					{selectedPaths}
					isLoading={isFileListLoading}
					compactMode={settings.compactMode}
					{cutPaths}
					{favoritePaths}
					{canPaste}
					{canCreate}
					showFileExtensions={settings.showFileExtensions}
					previewOnSingleClick={settings.previewOnSingleClick}
					onItemClick={handleFileClick}
					onSortChange={handleSortChange}
					onSelectionChange={handleSelectionChange}
					onContextMenuAction={handleContextMenuAction}
				/>
			{/if}
		</div>

		<!-- Status bar -->
		<StatusBar
			{itemCount}
			{selectedCount}
			{viewMode}
			totalCount={statusTotalCount}
			hasMore={hasMoreItems}
			{isLoadingMore}
			onLoadMore={handleLoadMore}
			onViewModeChange={handleViewModeChange}
		/>
	</div>
</div>

<!-- File Preview Modal -->
<FilePreview
	file={previewFile}
	allFiles={previewableFiles}
	onNavigate={handlePreviewNavigate}
	onFileSaved={handleFileSaved}
	onClose={handleClosePreview}
/>

<BrowseDialogs
	{createDialog}
	{renameDialog}
	{deleteDialog}
	{propertiesDialog}
	onCreateNameChange={(name) => (createDialog.name = name)}
	onRenameNameChange={(name) => (renameDialog.newName = name)}
	onCreateConfirm={() => void handleCreateConfirm()}
	onRenameConfirm={() => void handleRenameConfirm()}
	onDeleteConfirm={() => void handleDeleteConfirm()}
	onCloseCreate={closeCreateDialog}
	onCloseRename={() => (renameDialog = { open: false, file: null, newName: '' })}
	onCloseDelete={() => (deleteDialog = { open: false, items: [] })}
	onCloseProperties={() => (propertiesDialog = { open: false, file: null })}
/>

<!-- Hidden file input for upload button -->
<input
	bind:this={fileInputEl}
	type="file"
	multiple
	class="hidden"
	onchange={handleFileInputChange}
/>

<!-- Upload Panel (floating bottom-right) -->
<UploadPanel />

<!-- Toast notifications -->
<Toast />
