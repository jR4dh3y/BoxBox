<script lang="ts">
	/**
	 * Browse page - the file browser. The logic lives in `#lib/browse`; this page wires it to the UI.
	 */
	import { onMount, untrack } from 'svelte';
	import { createDropZone } from '#lib/browse/dropZone.svelte.js';
	import { useFileDrop } from '#lib/browse/fileDrop.js';
	import { browseRequests } from '#lib/browse/requests.svelte.js';
	import { watchDesktopDirectory } from '#lib/browse/desktopWatch.js';
	import { isNativePath } from '#lib/api/desktop.js';
	import type { BrowsePaneState } from '#lib/browse/pane.svelte.js';
	import type { RemoteCatalogSnapshot } from '#lib/browse/catalog.js';
	import type { BrowseRuntime } from '#lib/browse/runtime.js';
	import Sidebar from '#lib/components/Sidebar.svelte';
	import Toolbar from '#lib/components/Toolbar.svelte';
	import FileList from '#lib/components/FileList.svelte';
	import StatusBar from '#lib/components/StatusBar.svelte';
	import LoadError from '#lib/components/LoadError.svelte';
	import Notifications from '#lib/components/Notifications.svelte';
	import { Button } from '#lib/components/ui/index.js';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { uploadStore } from '#lib/stores/upload.svelte.js';
	import { jobsStore } from '#lib/stores/jobs.svelte.js';
	import { useBrowseActions } from '#lib/browse/actions.svelte.js';
	import { useBrowseData } from '#lib/browse/data.svelte.js';
	import { useBrowseLocation } from '#lib/browse/location.svelte.js';
	import { selectedFileBytes, useBrowseSelection } from '#lib/browse/selection.svelte.js';
	import { useBrowseUploads } from '#lib/browse/uploads.svelte.js';
	import { CONFIG } from '#lib/config.js';
	import { createHoverIntent } from '#lib/utils/hoverIntent.js';
	import type { AddAction } from '#lib/utils/addMenu.js';

	let {
		pane,
		catalog,
		onRuntime,
		onFocus,
		onToggleSidebar,
		focused = true
	}: {
		pane?: BrowsePaneState;
		catalog?: RemoteCatalogSnapshot;
		onRuntime?: (runtime: BrowseRuntime | null) => void;
		onFocus?: () => void;
		onToggleSidebar?: () => void;
		focused?: boolean;
	} = $props();
	const paneState = untrack(() => pane);
	const selection = paneState?.selection ?? useBrowseSelection();
	const location =
		paneState?.location ?? useBrowseLocation({ onLocationChange: () => selection.clear() });
	const data = useBrowseData(location, selection, () => catalog);
	onMount(() => {
		onRuntime?.({ location, data });
		return () => onRuntime?.(null);
	});
	const actions = useBrowseActions(location, data, selection);
	const uploads = useBrowseUploads(location, data);
	const dropHandler = useFileDrop(data, actions);
	const drop = createDropZone(() => dropHandler);
	const settings = $derived(settingsStore.current);
	let settingsOpen = $state(false);
	let settingsSection = $state<'general' | 'connections'>('general');
	let sidebarOpen = $state(false);
	let toolbar: Toolbar | undefined;
	function handleAdd(action: AddAction) {
		if (action === 'upload-files') uploads.openPicker();
		else if (action === 'upload-folder') uploads.openFolderPicker();
		else void actions.handleContextMenuAction(action, []);
	}
	const selectedBytes = $derived(selectedFileBytes(data.displayItems, selection.paths));
	$effect(() => {
		const path = location.path;
		if (paneState && path && isNativePath(path))
			return watchDesktopDirectory(path, () => {
				data.reload();
			});
	});
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

	function keydown(event: KeyboardEvent) {
		if (
			!focused ||
			event.defaultPrevented ||
			!(event.ctrlKey || event.metaKey) ||
			event.altKey ||
			event.shiftKey ||
			document.querySelector('[role="dialog"], dialog[open], [role="menu"]')
		)
			return;
		if (event.key.toLowerCase() === 'l') {
			event.preventDefault();
			toolbar?.focusLocation();
			return;
		}
		if (
			event.key.toLowerCase() === 'v' &&
			data.canCreate &&
			!(
				event.target instanceof Element &&
				event.target.closest('input, textarea, [contenteditable="true"]')
			)
		) {
			event.preventDefault();
			void actions.paste.paste();
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<svelte:head>
	<title>BoxBox</title>
</svelte:head>

<div
	class="browse-shell flex h-full min-h-0 w-full overflow-hidden bg-surface-primary"
	onpointerdown={onFocus}
	onfocusin={onFocus}
	role="region"
	aria-label="File browser pane"
>
	<!-- Sidebar -->
	{#if !paneState}<Sidebar
			currentPath={location.path}
			roots={data.roots}
			onNavigate={location.navigate}
			{onFolderHover}
			open={sidebarOpen}
			onClose={() => (sidebarOpen = false)}
		/>{/if}

	<!-- Main content area -->
	<div class="flex min-h-0 min-w-0 flex-1 flex-col">
		<!-- Toolbar with navigation and path bar -->
		<Toolbar
			bind:this={toolbar}
			pageTitle={paneState?.location.page === 'trash' ? 'Trash' : null}
			pathSegments={location.segments}
			roots={data.roots}
			serverName={data.serverName}
			canGoBack={location.canGoBack}
			canGoForward={location.canGoForward}
			onBack={location.back}
			onForward={location.forward}
			onNavigate={location.navigate}
			onRefresh={data.reload}
			onSettings={() => {
				settingsSection = 'general';
				settingsOpen = true;
			}}
			onToggleSidebar={() => (onToggleSidebar ? onToggleSidebar() : (sidebarOpen = !sidebarOpen))}
			onAdd={handleAdd}
			canCreate={data.canCreate}
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
			class="relative min-h-0 flex-1 overflow-hidden"
			ondragover={(event) => drop.over(event, location.path)}
			ondragleave={drop.leave}
			ondrop={(event) => drop.drop(event, location.path)}
			role="region"
			aria-label="File browser content"
		>
			<!-- Drag-drop overlay -->
			{#if drop.overPath === location.path && !data.isAtRoot}
				<div
					class="pointer-events-none absolute inset-0 z-20 flex items-center justify-center border-2 border-dashed border-accent bg-accent/10"
				>
					<div class="rounded-lg bg-surface-primary/90 px-6 py-4 shadow-lg backdrop-blur-sm">
						<span class="text-lg font-medium text-accent">Drop files to upload here</span>
					</div>
				</div>
			{/if}

			{#if paneState?.location.page === 'trash'}
				{#await import('./desktop/TrashPage.svelte') then { default: TrashPage }}<TrashPage
						onChanged={() => void browseRequests.changed()}
					/>{/await}
			{:else if data.isDeploymentRoot && data.deploymentError}
				<div class="space-y-3 p-6 text-sm text-text-secondary">
					<p>
						{data.deploymentError}
					</p>
					<Button
						onclick={() => {
							settingsSection = 'connections';
							settingsOpen = true;
						}}>Open Connections</Button
					>
				</div>
			{:else if data.isAtRoot}
				<!-- The drives page loads alongside the drive list, so folder pages never download it. -->
				{#await import('#lib/components/DrivesView.svelte') then { default: DrivesView }}
					<DrivesView
						drives={data.drives}
						isLoading={data.isLoadingDrives}
						viewMode={location.driveViewMode}
						compactMode={settings.compactMode}
						onOpen={(drive) => location.navigate(drive.name)}
						{onFolderHover}
					/>
				{:catch}
					<LoadError label="drives" inline />
				{/await}
			{:else if data.fileListError && data.displayItems.length === 0}
				<div role="alert" class="flex h-full items-center justify-center gap-3 p-6 text-sm">
					<span>{data.fileListError}</span>
					<button
						type="button"
						class="rounded border border-border-primary px-3 py-1 hover:bg-surface-secondary"
						onclick={data.reload}
					>
						Retry
					</button>
				</div>
			{:else if location.viewMode === 'grid'}
				{#await import('#lib/components/FileGrid.svelte') then { default: FileGrid }}
					<FileGrid
						viewKey={location.viewKey}
						dropTarget={drop}
						items={data.displayItems}
						emptyMessage={data.emptyListMessage}
						selectedPaths={selection.paths}
						isLoading={data.isFileListLoading}
						compactMode={settings.compactMode}
						cutPaths={actions.cutPaths}
						favoritePaths={actions.favoritePaths}
						canPaste={actions.canPaste}
						canCreate={data.canCreate}
						readOnly={data.isReadOnly}
						showFileExtensions={settings.showFileExtensions}
						previewOnSingleClick={settings.previewOnSingleClick}
						selectionCheckboxes={settings.selectionCheckboxes}
						onItemClick={actions.openFile}
						{onFolderHover}
						onSelectionChange={selection.set}
						onContextMenuAction={actions.handleContextMenuAction}
					/>
				{:catch}
					<LoadError label="grid view" inline />
				{/await}
			{:else}
				<FileList
					viewKey={location.viewKey}
					dropTarget={drop}
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
					readOnly={data.isReadOnly}
					showFileExtensions={settings.showFileExtensions}
					previewOnSingleClick={settings.previewOnSingleClick}
					selectionCheckboxes={settings.selectionCheckboxes}
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
			{selectedBytes}
			viewMode={data.isAtRoot ? location.driveViewMode : location.viewMode}
			onViewModeChange={location.setViewMode}
		/>
	</div>
</div>

{#if actions.previewFile && focused}
	{#await import('#lib/components/FilePreview.svelte') then { default: FilePreview }}
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
	{#await import('#lib/components/BrowseDialogs.svelte') then { default: BrowseDialogs }}
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

{#if actions.paste.plan}
	{#await import('#lib/components/desktop/TransferConflictDialog.svelte') then { default: TransferConflictDialog }}
		<TransferConflictDialog
			plan={actions.paste.plan}
			open
			onapply={actions.paste.apply}
			oncancelbatch={() => void actions.paste.cancel()}
		/>
	{/await}
{/if}

{#if actions.openWithFile}
	{#await import('./desktop/OpenWithDialog.svelte') then { default: OpenWithDialog }}<OpenWithDialog
			open
			path={actions.openWithFile.path}
			onclose={actions.closeOpenWith}
		/>{/await}
{/if}

{#if settingsOpen}
	{#await import('#lib/components/settings/SettingsModal.svelte') then { default: SettingsModal }}
		<SettingsModal initialSection={settingsSection} onclose={() => (settingsOpen = false)} />
	{:catch}
		<LoadError label="settings" onClose={() => (settingsOpen = false)} />
	{/await}
{/if}

{#if actions.shareItem}
	{#await import('#lib/components/ShareModal.svelte') then { default: ShareModal }}
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

<!-- Toasts, a failed refresh and the upload panel stack in one column. -->
{#if focused}<Notifications>
		{#if jobsStore.jobs.size}{#await import('./desktop/TransferJobs.svelte') then { default: TransferJobs }}<TransferJobs
				/>{/await}{/if}
		{#if data.fileListError && data.displayItems.length > 0}
			<!-- The stale list stays visible, so the failure is a notice instead of a page error. -->
			<div
				role="alert"
				class="pointer-events-auto flex items-center gap-3 rounded-lg border border-border-primary bg-surface-secondary py-2 pr-2 pl-4 text-[13px] shadow-lg"
			>
				<span class="min-w-0 flex-1 text-text-primary">{data.fileListError}</span>
				<Button variant="secondary" size="sm" onclick={data.reload}>Retry</Button>
			</div>
		{/if}
		{#if uploadStore.uploads.length > 0}
			{#await import('#lib/components/UploadPanel.svelte') then { default: UploadPanel }}
				<UploadPanel />
			{:catch}
				{#if !uploadErrorDismissed}
					<LoadError label="upload panel" onClose={() => (uploadErrorDismissed = true)} />
				{/if}
			{/await}
		{/if}
	</Notifications>{/if}
