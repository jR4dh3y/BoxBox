<script lang="ts">
	/**
	 * Browse page - the file browser. The logic lives in `$lib/browse`; this page wires it to the UI.
	 */
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import Toolbar from '$lib/components/Toolbar.svelte';
	import FileList from '$lib/components/FileList.svelte';
	import FileGrid from '$lib/components/FileGrid.svelte';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import DriveCard from '$lib/components/DriveCard.svelte';
	import LoadError from '$lib/components/LoadError.svelte';
	import Toast from '$lib/components/ui/Toast.svelte';
	import { Spinner } from '$lib/components/ui';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { uploadStore } from '$lib/stores/upload.svelte';
	import { useBrowseActions } from '$lib/browse/actions.svelte';
	import { useBrowseData } from '$lib/browse/data.svelte';
	import { useBrowseLocation } from '$lib/browse/location.svelte';
	import { useBrowseSelection } from '$lib/browse/selection.svelte';
	import { useBrowseUploads } from '$lib/browse/uploads.svelte';
	import { CONFIG } from '$lib/config';
	import { createHoverIntent } from '$lib/utils/hoverIntent';

	const selection = useBrowseSelection();
	const location = useBrowseLocation({ onLocationChange: () => selection.clear() });
	const data = useBrowseData(location);
	const actions = useBrowseActions(location, data, selection);
	const uploads = useBrowseUploads(location, data);
	const settings = $derived(settingsStore.current);
	// Uploads keep running without their panel, so closing the error only hides the message.
	let uploadErrorDismissed = $state(false);

	function closeDialogs() {
		actions.closeCreate();
		actions.closeRename();
		actions.closeDelete();
		actions.closeProperties();
	}
	// Overlays load on first use, so they stay out of the browse page's first download.
	const hasOpenDialog = $derived(
		actions.createDialog.open ||
			actions.renameDialog.open ||
			actions.deleteDialog.open ||
			actions.propertiesDialog.open
	);
	const onFolderHover = createHoverIntent(
		data.prefetchDirectory,
		CONFIG.query.prefetchHoverDelayMs
	);
	// A removed row never fires pointerleave, so drop a pending prefetch on navigation and teardown.
	$effect(() => {
		void location.path;
		return () => onFolderHover(null);
	});
</script>

<svelte:head>
	<title>BoxBox</title>
</svelte:head>

<div class="flex h-screen w-full overflow-hidden bg-surface-primary">
	<!-- Sidebar -->
	<Sidebar
		currentPath={location.path}
		roots={data.roots}
		onNavigate={location.navigate}
		{onFolderHover}
	/>

	<!-- Main content area -->
	<div class="flex min-w-0 flex-1 flex-col">
		<!-- Toolbar with navigation and path bar -->
		<Toolbar
			pathSegments={location.segments}
			canGoBack={location.canGoBack}
			canGoForward={location.canGoForward}
			canGoUp={location.canGoUp}
			onBack={location.back}
			onForward={location.forward}
			onUp={location.up}
			onNavigate={location.navigate}
			onRefresh={data.reload}
			onSettings={() => goto(resolve('/settings'))}
			onUpload={uploads.openPicker}
			onUploadFolder={uploads.openFolderPicker}
			uploadDisabled={data.uploadDisabled}
			showSearch={!data.isAtRoot}
			searchValue={location.searchQuery}
			searchLoading={data.isSearching}
			onSearchInput={location.search}
			onSearchClear={location.clearSearch}
			includeHiddenSuggestions={settings.showHiddenFiles}
		/>

		<!-- File list or Drive cards -->
		<div
			class="relative flex-1 overflow-auto"
			ondragover={uploads.handleDragOver}
			ondragleave={uploads.handleDragLeave}
			ondrop={uploads.handleDrop}
			role="region"
			aria-label="File browser content"
		>
			<!-- Drag-drop overlay -->
			{#if uploads.isDragOver && !data.isAtRoot}
				<div
					class="pointer-events-none absolute inset-0 z-20 flex items-center justify-center border-2 border-dashed border-accent bg-accent/10"
				>
					<div class="rounded-lg bg-surface-primary/90 px-6 py-4 shadow-lg backdrop-blur-sm">
						<span class="text-lg font-medium text-accent">Drop files to upload here</span>
					</div>
				</div>
			{/if}

			{#if data.isAtRoot}
				<!-- This Server view - show drive cards -->
				<div class="p-6">
					{#if data.isLoadingDrives}
						<div class="flex items-center gap-2 py-5 text-sm text-text-secondary">
							<Spinner size="sm" />
							<span>Loading drives...</span>
						</div>
					{:else if data.drives.length === 0}
						<div class="py-5 text-sm text-text-secondary">No configured storage found</div>
					{:else}
						<div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
							{#each data.drives as drive (drive.name)}
								<DriveCard {drive} onClick={() => location.navigate(drive.name)} {onFolderHover} />
							{/each}
						</div>
					{/if}
				</div>
			{:else if location.viewMode === 'grid'}
				<FileGrid
					items={data.displayItems}
					emptyMessage={data.emptyListMessage}
					selectedPaths={selection.paths}
					isLoading={data.isFileListLoading}
					compactMode={settings.compactMode}
					cutPaths={actions.cutPaths}
					favoritePaths={actions.favoritePaths}
					canPaste={actions.canPaste}
					canCreate={data.canCreate}
					showFileExtensions={settings.showFileExtensions}
					previewOnSingleClick={settings.previewOnSingleClick}
					onItemClick={actions.openFile}
					{onFolderHover}
					onSelectionChange={selection.set}
					onContextMenuAction={actions.handleContextMenuAction}
				/>
			{:else}
				<FileList
					items={data.displayItems}
					sortBy={location.sortBy}
					sortDir={location.sortDir}
					emptyMessage={data.emptyListMessage}
					selectedPaths={selection.paths}
					isLoading={data.isFileListLoading}
					compactMode={settings.compactMode}
					cutPaths={actions.cutPaths}
					favoritePaths={actions.favoritePaths}
					canPaste={actions.canPaste}
					canCreate={data.canCreate}
					showFileExtensions={settings.showFileExtensions}
					previewOnSingleClick={settings.previewOnSingleClick}
					onItemClick={actions.openFile}
					{onFolderHover}
					onSortChange={location.setSort}
					onSelectionChange={selection.set}
					onContextMenuAction={actions.handleContextMenuAction}
				/>
			{/if}
		</div>

		<!-- Status bar -->
		<StatusBar
			itemCount={data.itemCount}
			selectedCount={selection.count}
			viewMode={location.viewMode}
			totalCount={data.statusTotalCount}
			hasMore={data.hasMoreItems}
			isLoadingMore={data.isLoadingMore}
			onLoadMore={data.loadMore}
			onViewModeChange={location.setViewMode}
		/>
	</div>
</div>

{#if actions.previewFile}
	{#await import('$lib/components/FilePreview.svelte') then { default: FilePreview }}
		<FilePreview
			file={actions.previewFile}
			allFiles={data.previewableFiles}
			onNavigate={actions.showPreview}
			onFileSaved={actions.fileSaved}
			onClose={actions.closePreview}
		/>
	{:catch}
		<LoadError label="preview" onClose={actions.closePreview} />
	{/await}
{/if}

{#if hasOpenDialog}
	{#await import('$lib/components/BrowseDialogs.svelte') then { default: BrowseDialogs }}
		<BrowseDialogs
			createDialog={actions.createDialog}
			renameDialog={actions.renameDialog}
			deleteDialog={actions.deleteDialog}
			propertiesDialog={actions.propertiesDialog}
			onCreateNameChange={actions.setCreateName}
			onRenameNameChange={actions.setRenameName}
			onCreateConfirm={() => void actions.confirmCreate()}
			onRenameConfirm={() => void actions.confirmRename()}
			onDeleteConfirm={() => void actions.confirmDelete()}
			onCloseCreate={actions.closeCreate}
			onCloseRename={actions.closeRename}
			onCloseDelete={actions.closeDelete}
			onCloseProperties={actions.closeProperties}
		/>
	{:catch}
		<LoadError label="dialog" onClose={closeDialogs} />
	{/await}
{/if}

{#if actions.shareItem}
	{#await import('$lib/components/ShareModal.svelte') then { default: ShareModal }}
		<ShareModal open item={actions.shareItem} onclose={actions.closeShare} />
	{:catch}
		<LoadError label="share window" onClose={actions.closeShare} />
	{/await}
{/if}

<!-- Hidden file input for upload button -->
<input
	bind:this={uploads.fileInput}
	type="file"
	multiple
	class="hidden"
	onchange={uploads.handleInputChange}
/>
<input
	bind:this={uploads.folderInput}
	type="file"
	webkitdirectory
	class="hidden"
	onchange={uploads.handleInputChange}
/>

<!-- Upload Panel (floating bottom-right) -->
{#if uploadStore.uploads.length > 0}
	{#await import('$lib/components/UploadPanel.svelte') then { default: UploadPanel }}
		<UploadPanel />
	{:catch}
		{#if !uploadErrorDismissed}
			<LoadError label="upload panel" onClose={() => (uploadErrorDismissed = true)} />
		{/if}
	{/await}
{/if}

<!-- Toast notifications -->
<Toast />
