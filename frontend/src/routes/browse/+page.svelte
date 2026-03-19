<script lang="ts">
	/**
	 * Browse page - main file browser interface (FilePilot style)
	 */
	import { createQuery } from '@tanstack/svelte-query';
	import { goto } from '$app/navigation';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import Toolbar from '$lib/components/Toolbar.svelte';
	import SearchBar from '$lib/components/SearchBar.svelte';
	import FileList from '$lib/components/FileList.svelte';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import SystemDriveCard from '$lib/components/SystemDriveCard.svelte';
	import FilePreview from '$lib/components/FilePreview.svelte';
	import UploadPanel from '$lib/components/UploadPanel.svelte';
	import Toast from '$lib/components/ui/Toast.svelte';
	import { Spinner, Modal, Input, Button } from '$lib/components/ui';
	import { pathStore, listOptionsStore, fileQueryKeys } from '$lib/stores/files.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { clipboardStore } from '$lib/stores/clipboard.svelte';
	import { uploadStore } from '$lib/stores/upload.svelte';
	import { toastStore } from '$lib/stores/toast.svelte';
	import {
		listRoots,
		listDirectory,
		search,
		rename,
		deleteFile,
		getDownloadUrl
	} from '$lib/api/files';
	import { getSystemDrives, type SystemDrivesResponse } from '$lib/api/system';
	import { createCopyJob, createMoveJob, createDeleteJob } from '$lib/api/jobs';
	import { formatFileSize, formatFileDate, mapSystemMountToBrowsePath } from '$lib/utils/format';
	import type { SortField, SortDir } from '$lib/types/files';
	import type {
		FileInfo,
		FileList as FileListType,
		RootsResponse,
		SearchResponse
	} from '$lib/api/files';
	import { SvelteSet } from 'svelte/reactivity';

	let searchQuery = $state('');
	let selectedPaths = $state(new Set<string>());
	let viewMode = $state<'list' | 'grid'>('list');
	let previewFile = $state<FileInfo | null>(null);
	let historyStack = $state<string[]>(['']);
	let historyIndex = $state(0);

	// Upload state
	let fileInputEl: HTMLInputElement;
	let isDragOver = $state(false);

	// Rename dialog state
	let renameDialog = $state<{ open: boolean; file: FileInfo | null; newName: string }>({
		open: false,
		file: null,
		newName: ''
	});

	// Properties dialog state
	let propertiesDialog = $state<{ open: boolean; file: FileInfo | null }>({
		open: false,
		file: null
	});

	const path = $derived(pathStore.currentPath);
	const segments = $derived(pathStore.pathSegments);
	const options = $derived(listOptionsStore.value);
	const settings = $derived(settingsStore.value);
	const trimmedSearchQuery = $derived(searchQuery.trim());
	const isSearchActive = $derived(trimmedSearchQuery.length >= 2);

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
		queryKey: fileQueryKeys.list(path, options),
		queryFn: () => listDirectory(path, options),
		enabled: path !== ''
	}));

	const searchQueryResult = createQuery<SearchResponse>(() => ({
		queryKey: fileQueryKeys.search(path, trimmedSearchQuery),
		queryFn: () => search(path, trimmedSearchQuery),
		enabled: path !== '' && isSearchActive
	}));

	const isLoading = $derived(directoryQuery.isLoading);
	const isFileListLoading = $derived(isSearchActive ? searchQueryResult.isFetching : isLoading);
	const fileList = $derived(directoryQuery.data ?? null);
	const searchResults = $derived(searchQueryResult.data?.results ?? []);
	const roots = $derived(rootsQuery.data?.roots ?? []);
	const systemDrives = $derived(systemDrivesQuery.data?.drives ?? []);
	const isAtRoot = $derived(path === '');

	// Clipboard state for context menu
	const canPaste = $derived(clipboardStore.hasItems);
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
			items = fileList?.items ?? [];
		}

		if (!settings.showHiddenFiles) {
			items = items.filter((item) => !item.name.startsWith('.'));
		}

		return items;
	});
	const previewableFiles = $derived(displayItems.filter((item) => !item.isDir));
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

	function handleNavigate(newPath: string) {
		const newHistory = historyStack.slice(0, historyIndex + 1);
		newHistory.push(newPath);
		historyStack = newHistory;
		historyIndex = newHistory.length - 1;

		pathStore.navigateTo(newPath);
		searchQuery = '';
		selectedPaths = new Set();
	}

	function handleBack() {
		if (canGoBack) {
			historyIndex--;
			pathStore.navigateTo(historyStack[historyIndex]);
			selectedPaths = new Set();
		}
	}

	function handleForward() {
		if (canGoForward) {
			historyIndex++;
			pathStore.navigateTo(historyStack[historyIndex]);
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
		goto('/settings');
	}

	function handleFileClick(file: FileInfo) {
		if (file.isDir) {
			handleNavigate(file.path);
		} else {
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

	function handleViewModeChange(mode: 'list' | 'grid') {
		viewMode = mode;
	}

	/**
	 * Handle context menu actions
	 */
	async function handleContextMenuAction(action: string, items: FileInfo[]) {
		switch (action) {
			case 'copy':
				clipboardStore.copy(items);
				break;

			case 'cut':
				clipboardStore.cut(items);
				break;

			case 'paste':
				await handlePaste();
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
				await handleDeleteItems(items);
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

	async function handleDeleteItems(items: FileInfo[]) {
		if (items.length === 0) {
			return;
		}

		if (settings.confirmDelete) {
			const itemLabel =
				items.length === 1
					? `Delete "${items[0].name}"?`
					: `Delete ${items.length} selected items?`;
			if (!window.confirm(itemLabel)) {
				return;
			}
		}

		try {
			for (const item of items) {
				if (item.isDir) {
					await createDeleteJob(item.path);
				} else {
					await deleteFile(item.path);
				}
			}

			selectedPaths = new Set();
			directoryQuery.refetch();
		} catch (error) {
			console.error('Delete failed:', error);
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
					await createCopyJob(item.path, destPath);
				} else if (operation === 'cut') {
					await createMoveJob(item.path, destPath);
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
		}
	}

	/**
	 * Handle delete confirmation
	 */
	// Setup upload store callbacks
	uploadStore.onComplete = (fileName: string, success: boolean, error?: string) => {
		if (success) {
			toastStore.success(`${fileName} uploaded successfully`);
		} else {
			toastStore.error(`Upload failed: ${error || 'Unknown error'}`);
		}
	};

	uploadStore.onRefreshNeeded = () => {
		directoryQuery.refetch();
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
		const currentMount = roots.find((r) => path.startsWith(r.name));
		if (currentMount?.readOnly) {
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
		const currentMount = roots.find((r) => path.startsWith(r.name));
		if (currentMount?.readOnly) {
			toastStore.error('Cannot upload to read-only location');
			return;
		}

		uploadStore.addFiles(files, path);
	}

	// Derived: is upload disabled (at root or read-only)
	const uploadDisabled = $derived.by(() => {
		if (isAtRoot) return true;
		const currentMount = roots.find((r) => path.startsWith(r.name));
		return currentMount?.readOnly ?? false;
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
		/>

		{#if !isAtRoot}
			<div class="border-b border-border-secondary bg-surface-primary px-3 py-2">
				<SearchBar
					value={searchQuery}
					onInput={handleSearchInput}
					onClear={handleSearchClear}
					isLoading={isSearchActive && searchQueryResult.isFetching}
					placeholder="Search files and folders..."
				/>
			</div>
		{/if}

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
					<h2 class="m-0 mb-5 text-lg font-medium text-text-primary">Storage Devices</h2>
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
					{canPaste}
					onItemClick={handleFileClick}
					onSortChange={handleSortChange}
					onSelectionChange={handleSelectionChange}
					onContextMenuAction={handleContextMenuAction}
				/>
			{/if}
		</div>

		<!-- Status bar -->
		<StatusBar {itemCount} {selectedCount} {viewMode} onViewModeChange={handleViewModeChange} />
	</div>
</div>

<!-- File Preview Modal -->
<FilePreview
	file={previewFile}
	allFiles={previewableFiles}
	onNavigate={handlePreviewNavigate}
	onClose={handleClosePreview}
/>

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
