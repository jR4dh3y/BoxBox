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
	import FilePreview from '$lib/components/FilePreview.svelte';
	import BrowseDialogs from '$lib/components/BrowseDialogs.svelte';
	import ShareModal from '$lib/components/ShareModal.svelte';
	import FolderShareModal from '$lib/components/FolderShareModal.svelte';
	import UploadPanel from '$lib/components/UploadPanel.svelte';
	import Toast from '$lib/components/ui/Toast.svelte';
	import { Spinner } from '$lib/components/ui';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import { useBrowseActions } from '$lib/browse/actions.svelte';
	import { useBrowseData } from '$lib/browse/data.svelte';
	import { useBrowseLocation } from '$lib/browse/location.svelte';
	import { useBrowseSelection } from '$lib/browse/selection.svelte';
	import { useBrowseUploads } from '$lib/browse/uploads.svelte';

	const selection = useBrowseSelection();
	const location = useBrowseLocation({ onLocationChange: () => selection.clear() });
	const data = useBrowseData(location);
	const actions = useBrowseActions(location, data, selection);
	const uploads = useBrowseUploads(location, data);
	const settings = $derived(settingsStore.current);
</script>

<svelte:head>
	<title>BoxBox</title>
</svelte:head>

<div class="flex h-screen w-full overflow-hidden bg-surface-primary">
	<!-- Sidebar -->
	<Sidebar currentPath={location.path} roots={data.roots} onNavigate={location.navigate} />

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
								<DriveCard {drive} onClick={() => location.navigate(drive.name)} />
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

<!-- File Preview Modal -->
<FilePreview
	file={actions.previewFile}
	allFiles={data.previewableFiles}
	onNavigate={actions.showPreview}
	onFileSaved={actions.fileSaved}
	onClose={actions.closePreview}
/>

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

<!-- Share Modal -->
<ShareModal
	open={actions.shareDialog.open}
	file={actions.shareDialog.file}
	onclose={actions.closeShare}
/>
<FolderShareModal
	open={actions.folderShareDialog.open}
	folder={actions.folderShareDialog.folder}
	onclose={actions.closeFolderShare}
/>

<!-- Hidden file input for upload button -->
<input
	bind:this={uploads.fileInput}
	type="file"
	multiple
	class="hidden"
	onchange={uploads.handleInputChange}
/>

<!-- Upload Panel (floating bottom-right) -->
<UploadPanel />

<!-- Toast notifications -->
<Toast />
