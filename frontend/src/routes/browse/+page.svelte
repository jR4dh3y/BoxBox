<script lang="ts">
	/**
	 * Browse page - main file browser interface (FilePilot style)
	 */
	import { createQuery, useQueryClient } from '@tanstack/svelte-query';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import Toolbar from '$lib/components/Toolbar.svelte';
	import FileList from '$lib/components/FileList.svelte';
	import FileGrid from '$lib/components/FileGrid.svelte';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import SystemDriveCard from '$lib/components/SystemDriveCard.svelte';
	import FilePreview from '$lib/components/FilePreview.svelte';
	import UploadPanel from '$lib/components/UploadPanel.svelte';
	import Toast from '$lib/components/ui/Toast.svelte';
	import { Spinner, Modal, Input, Button } from '$lib/components/ui';
	import {
		pathStore,
		currentPath,
		pathSegments,
		listOptionsStore,
		fileQueryKeys
	} from '$lib/stores/files';
	import { settingsStore } from '$lib/stores/settings';
	import { clipboardStore } from '$lib/stores/clipboard.svelte';
	import { uploadStore } from '$lib/stores/upload.svelte';
	import { toastStore } from '$lib/stores/toast.svelte';
	import { jobsStore } from '$lib/stores/jobs';
	import {
		listRoots,
		listDirectory,
		search,
		createDirectory,
		createFile,
		rename,
		deleteFile,
		getDownloadUrl
	} from '$lib/api/files';
	import { getSystemDrives, type SystemDrivesResponse } from '$lib/api/system';
	import { createCopyJob, createMoveJob, createDeleteJob } from '$lib/api/jobs';
	import { formatFileSize, formatFileDate, mapSystemMountToBrowsePath } from '$lib/utils/format';
	import { canPreview, getFileTypeDescription } from '$lib/utils/fileTypes';
	import type { SortField, SortDir, ViewMode } from '$lib/types/files';
	import type {
		FileInfo,
		FileList as FileListType,
		RootsResponse,
		SearchResponse
	} from '$lib/api/files';
	import { untrack } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';

	let searchQuery = $state('');
	let selectedPaths = $state(new Set<string>());
	let viewMode = $state<ViewMode>(settingsStore.getSetting('defaultViewMode'));
	let previewFile = $state<FileInfo | null>(null);
	let historyStack = $state<string[]>(['']);
	let historyIndex = $state(0);
	let loadedFileItems = $state<FileInfo[]>([]);
	let loadedTotalCount = $state(0);
	let activeListScope = $state('');

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

	const path = $derived($currentPath);
	const segments = $derived($pathSegments);
	const options = $derived($listOptionsStore);
	const settings = $derived($settingsStore);
	const trimmedSearchQuery = $derived(searchQuery.trim());
	const isSearchActive = $derived(trimmedSearchQuery.length >= 2);
	const directoryOptions = $derived({ ...options, includeHidden: settings.showHiddenFiles });
	const listScope = $derived(
		[
			path,
			directoryOptions.pageSize,
			directoryOptions.sortBy,
			directoryOptions.sortDir,
			directoryOptions.filter,
			directoryOptions.includeHidden
		].join('\u0000')
	);
	const queryClient = useQueryClient();

	const rootsQuery = createQuery<RootsResponse>(() => ({
		queryKey: fileQueryKeys.roots(),
		queryFn: () => listRoots()
	}));

	const systemDrivesQuery = createQuery<SystemDrivesResponse>(() => ({
		queryKey: ['system', 'drives'],
		queryFn: () => getSystemDrives(),
		enabled: path === ''
	}));

	const directoryQuery = createQuery<FileListType>(() => ({
		queryKey: fileQueryKeys.list(path, directoryOptions),
		queryFn: () => listDirectory(path, directoryOptions),
		enabled: path !== ''
	}));

	const searchQueryResult = createQuery<SearchResponse>(() => ({
		queryKey: fileQueryKeys.search(path, trimmedSearchQuery),
		queryFn: () => search(path, trimmedSearchQuery),
		enabled: path !== '' && isSearchActive
	}));

	const isLoading = $derived(directoryQuery.isLoading);
	const isLoadingMore = $derived(!isSearchActive && directoryQuery.isFetching && options.page > 1);
	const isFileListLoading = $derived(
		isSearchActive ? searchQueryResult.isFetching : isLoading && loadedFileItems.length === 0
	);
	const fileList = $derived(directoryQuery.data ?? null);
	const searchResults = $derived(searchQueryResult.data?.results ?? []);
	const roots = $derived(rootsQuery.data?.roots ?? []);
	const systemDrives = $derived(systemDrivesQuery.data?.drives ?? []);
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
			items = loadedFileItems;
		}

		if (!settings.showHiddenFiles) {
			items = items.filter((item) => !item.name.startsWith('.'));
		}

		return items;
	});
	const previewableFiles = $derived(
		displayItems.filter((item) => !item.isDir && canPreview(item.name))
	);
	const hasMoreItems = $derived(
		!isSearchActive && !isAtRoot && loadedFileItems.length < loadedTotalCount
	);
	const statusTotalCount = $derived.by(() => {
		if (isAtRoot || isSearchActive) return undefined;
		return loadedTotalCount;
	});
	const emptyListMessage = $derived.by(() => {
		if (isSearchActive) {
			return `No matches for "${trimmedSearchQuery}" in this folder`;
		}
		return 'This folder is empty';
	});

	const itemCount = $derived(isAtRoot ? systemDrives.length : displayItems.length);
	const selectedCount = $derived(selectedPaths.size);
	const canGoBack = $derived(historyIndex > 0);
	const canGoForward = $derived(historyIndex < historyStack.length - 1);
	const canGoUp = $derived(segments.length > 0);
	const currentMount = $derived(
		path ? roots.find((root) => path === root.name || path.startsWith(`${root.name}/`)) : null
	);
	const isCurrentLocationReadOnly = $derived(currentMount?.readOnly ?? false);
	const canCreate = $derived(!isAtRoot && !isCurrentLocationReadOnly);

	$effect(() => {
		if (listScope === activeListScope) return;

		activeListScope = listScope;
		loadedFileItems = [];
		loadedTotalCount = 0;

		if (options.page !== 1) {
			listOptionsStore.setPage(1);
		}
	});

	$effect(() => {
		if (!fileList || fileList.path !== path) return;

		untrack(() => {
			loadedTotalCount = fileList.totalCount;

			if (fileList.page <= 1) {
				loadedFileItems = fileList.items;
				return;
			}

			const seenPaths = new Set(loadedFileItems.map((item) => item.path));
			const newItems = fileList.items.filter((item) => !seenPaths.has(item.path));
			if (newItems.length > 0) {
				loadedFileItems = [...loadedFileItems, ...newItems];
			}
		});
	});

	function getErrorMessage(error: unknown, fallback: string): string {
		return error instanceof Error ? error.message : fallback;
	}

	function handleNavigate(newPath: string) {
		const newHistory = historyStack.slice(0, historyIndex + 1);
		newHistory.push(newPath);
		historyStack = newHistory;
		historyIndex = newHistory.length - 1;

		pathStore.navigateTo(newPath);
		listOptionsStore.setPage(1);
		searchQuery = '';
		selectedPaths = new Set();
	}

	function handleBack() {
		if (canGoBack) {
			historyIndex--;
			pathStore.navigateTo(historyStack[historyIndex]);
			listOptionsStore.setPage(1);
			selectedPaths = new Set();
		}
	}

	function handleForward() {
		if (canGoForward) {
			historyIndex++;
			pathStore.navigateTo(historyStack[historyIndex]);
			listOptionsStore.setPage(1);
			selectedPaths = new Set();
		}
	}

	function handleUp() {
		if (canGoUp) {
			const parentPath = segments.slice(0, -1).join('/');
			handleNavigate(parentPath);
		}
	}

	function handleRefresh() {
		if (isAtRoot) {
			systemDrivesQuery.refetch();
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
		searchQuery = query;
	}

	function handleSearchClear() {
		searchQuery = '';
	}

	function handleSortChange(field: SortField, dir: SortDir) {
		listOptionsStore.setSortBy(field);
		listOptionsStore.setSortDir(dir);
	}

	function handleSelectionChange(paths: Set<string>) {
		selectedPaths = paths;
	}

	function handleViewModeChange(mode: ViewMode) {
		viewMode = mode;
	}

	function handleLoadMore() {
		if (!hasMoreItems || directoryQuery.isFetching) return;
		listOptionsStore.setPage(options.page + 1);
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
				deleteDialog = {
					open: true,
					items: items
				};
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

	async function handleCreateConfirm() {
		if (!path || !createDialog.name.trim()) return;

		const name = createDialog.name.trim();
		if (name.includes('/') || name.includes('\\')) {
			toastStore.error('Name cannot contain path separators');
			return;
		}

		try {
			const created =
				createDialog.type === 'file'
					? await createFile(path, name)
					: await createDirectory(path, name);

			closeCreateDialog();
			selectedPaths = new Set([created.path]);
			toastStore.success(`${created.name} created`);
			queryClient.invalidateQueries({ queryKey: fileQueryKeys.all });
			directoryQuery.refetch();
			if (isSearchActive) {
				searchQueryResult.refetch();
			}
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
		if (!renameDialog.file || !renameDialog.newName.trim()) return;

		const oldPath = renameDialog.file.path;
		const parentPath = oldPath.substring(0, oldPath.lastIndexOf('/'));
		const newPath = parentPath ? `${parentPath}/${renameDialog.newName}` : renameDialog.newName;

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
	 * Handle delete confirmation
	 */
	async function handleDeleteConfirm() {
		if (deleteDialog.items.length === 0) return;

		try {
			for (const item of deleteDialog.items) {
				if (item.isDir) {
					// Use job for directory deletion
					jobsStore.upsertJob(await createDeleteJob(item.path));
				} else {
					await deleteFile(item.path);
				}
			}
			deleteDialog = { open: false, items: [] };
			selectedPaths = new Set();
			directoryQuery.refetch();
		} catch (error) {
			console.error('Delete failed:', error);
			toastStore.error(getErrorMessage(error, 'Delete failed'));
		}
	}

	function handleFileSaved(file: FileInfo) {
		previewFile = file;
		queryClient.invalidateQueries({ queryKey: fileQueryKeys.all });
		directoryQuery.refetch();
		if (isSearchActive) {
			searchQueryResult.refetch();
		}
	}

	// Setup upload store callbacks
	uploadStore.onComplete = (fileName: string, success: boolean, error?: string) => {
		if (success) {
			toastStore.success(`${fileName} uploaded successfully`);
		} else {
			toastStore.error(`Upload failed: ${error || 'Unknown error'}`);
		}
	};

	uploadStore.onRefreshNeeded = () => {
		queryClient.invalidateQueries({ queryKey: fileQueryKeys.all });
		directoryQuery.refetch();
		if (isSearchActive) {
			searchQueryResult.refetch();
		}
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
	<title>File Manager</title>
</svelte:head>

<div class="flex h-screen w-full overflow-hidden bg-surface-primary">
	<!-- Sidebar -->
	<Sidebar {roots} currentPath={path} onNavigate={handleNavigate} />

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
					{#if systemDrivesQuery.isLoading}
						<div class="flex items-center gap-2 py-5 text-sm text-text-secondary">
							<Spinner size="sm" />
							<span>Loading drives...</span>
						</div>
					{:else if systemDrives.length === 0}
						<div class="py-5 text-sm text-text-secondary">No storage devices found</div>
					{:else}
						<div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
							{#each systemDrives as drive (drive.mountPoint)}
								<SystemDriveCard
									{drive}
									onClick={() => {
										// Map system mount point to browsable path via host root mount
										const browsePath = mapSystemMountToBrowsePath(drive.mountPoint);

										handleNavigate(browsePath);
									}}
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
					onItemClick={handleFileClick}
					onSelectionChange={handleSelectionChange}
					onContextMenuAction={handleContextMenuAction}
				/>
			{:else}
				<FileList
					items={displayItems}
					sortBy={options.sortBy}
					sortDir={options.sortDir}
					emptyMessage={emptyListMessage}
					{selectedPaths}
					isLoading={isFileListLoading}
					compactMode={settings.compactMode}
					{cutPaths}
					{favoritePaths}
					{canPaste}
					{canCreate}
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

<!-- Create File/Folder Dialog -->
<Modal
	open={createDialog.open}
	title={createDialog.type === 'file' ? 'New File' : 'New Folder'}
	onclose={closeCreateDialog}
>
	<div class="flex flex-col gap-4">
		<p class="text-sm text-text-secondary">
			Enter a name for the new {createDialog.type === 'file' ? 'file' : 'folder'}:
		</p>
		<Input
			bind:value={createDialog.name}
			placeholder={createDialog.type === 'file' ? 'untitled.txt' : 'New Folder'}
			onkeydown={(e) => e.key === 'Enter' && handleCreateConfirm()}
		/>
	</div>
	{#snippet footer()}
		<Button variant="secondary" onclick={closeCreateDialog}>Cancel</Button>
		<Button variant="primary" onclick={handleCreateConfirm}>
			Create {createDialog.type === 'file' ? 'File' : 'Folder'}
		</Button>
	{/snippet}
</Modal>

<!-- Rename Dialog -->
<Modal
	open={renameDialog.open}
	title="Rename"
	onclose={() => (renameDialog = { open: false, file: null, newName: '' })}
>
	<div class="flex flex-col gap-4">
		<p class="text-sm text-text-secondary">Enter a new name:</p>
		<Input
			bind:value={renameDialog.newName}
			placeholder="New name"
			onkeydown={(e) => e.key === 'Enter' && handleRenameConfirm()}
		/>
	</div>
	{#snippet footer()}
		<Button
			variant="secondary"
			onclick={() => (renameDialog = { open: false, file: null, newName: '' })}
		>
			Cancel
		</Button>
		<Button variant="primary" onclick={handleRenameConfirm}>Rename</Button>
	{/snippet}
</Modal>

<!-- Delete Confirmation Dialog -->
<Modal
	open={deleteDialog.open}
	title="Delete"
	onclose={() => (deleteDialog = { open: false, items: [] })}
>
	<div class="flex flex-col gap-3 text-sm text-text-secondary">
		<p>
			Delete {deleteDialog.items.length}
			{deleteDialog.items.length === 1 ? 'item' : 'items'}?
		</p>
		{#if deleteDialog.items.length > 0}
			<ul class="max-h-40 list-none overflow-auto rounded border border-border-secondary p-0">
				{#each deleteDialog.items as item (item.path)}
					<li class="border-b border-border-secondary px-3 py-2 last:border-b-0">
						<span class="block truncate text-text-primary" title={item.path}>{item.name}</span>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
	{#snippet footer()}
		<Button variant="secondary" onclick={() => (deleteDialog = { open: false, items: [] })}>
			Cancel
		</Button>
		<Button variant="danger" onclick={handleDeleteConfirm}>Delete</Button>
	{/snippet}
</Modal>

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

<!-- Properties Dialog -->
<Modal
	open={propertiesDialog.open}
	title="Properties"
	onclose={() => (propertiesDialog = { open: false, file: null })}
>
	{#if propertiesDialog.file}
		{@const file = propertiesDialog.file}
		<div class="flex flex-col gap-3 text-sm">
			<div class="flex justify-between">
				<span class="text-text-secondary">Name:</span>
				<span class="font-medium text-text-primary">{file.name}</span>
			</div>
			<div class="flex justify-between">
				<span class="text-text-secondary">Type:</span>
				<span class="text-text-primary">{file.isDir ? 'Folder' : file.mimeType || 'File'}</span>
			</div>
			<div class="flex justify-between">
				<span class="text-text-secondary">Path:</span>
				<span class="break-all text-text-primary">{file.path}</span>
			</div>
			{#if !file.isDir}
				<div class="flex justify-between">
					<span class="text-text-secondary">Size:</span>
					<span class="text-text-primary">{formatFileSize(file.size)}</span>
				</div>
			{/if}
			<div class="flex justify-between">
				<span class="text-text-secondary">Modified:</span>
				<span class="text-text-primary">{formatFileDate(file.modTime)}</span>
			</div>
			<div class="flex justify-between">
				<span class="text-text-secondary">Permissions:</span>
				<span class="font-mono text-text-primary">{file.permissions}</span>
			</div>
		</div>
	{/if}
	{#snippet footer()}
		<Button variant="secondary" onclick={() => (propertiesDialog = { open: false, file: null })}>
			Close
		</Button>
	{/snippet}
</Modal>
