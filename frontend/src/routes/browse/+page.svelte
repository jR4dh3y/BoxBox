<script lang="ts">
	/**
	 * Browse page - the file browser. The logic lives in `#lib/browse`; this page wires it to the UI.
	 */
	import Sidebar from '#lib/components/Sidebar.svelte';
	import Toolbar from '#lib/components/Toolbar.svelte';
	import FileList from '#lib/components/FileList.svelte';
	import StatusBar from '#lib/components/StatusBar.svelte';
	import LoadError from '#lib/components/LoadError.svelte';
	import Notifications from '#lib/components/Notifications.svelte';
	import { Button } from '#lib/components/ui/index.js';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { uploadStore } from '#lib/stores/upload.svelte.js';
	import { useBrowseActions } from '#lib/browse/actions.svelte.js';
	import { useBrowseData } from '#lib/browse/data.svelte.js';
	import { useBrowseLocation } from '#lib/browse/location.svelte.js';
	import { selectedFileBytes, useBrowseSelection } from '#lib/browse/selection.svelte.js';
	import { useBrowseUploads } from '#lib/browse/uploads.svelte.js';
	import { CONFIG } from '#lib/config.js';
	import { createHoverIntent } from '#lib/utils/hoverIntent.js';
	import type { AddAction } from '#lib/utils/addMenu.js';

	const selection = useBrowseSelection();
	const location = useBrowseLocation({ onLocationChange: () => selection.clear() });
	const data = useBrowseData(location, selection);
	const actions = useBrowseActions(location, data, selection);
	const uploads = useBrowseUploads(location, data);
	const settings = $derived(settingsStore.current);
	let settingsOpen = $state(false);
	let sidebarOpen = $state(false);
	function handleAdd(action: AddAction) {
		if (action === 'upload-files') uploads.openPicker();
		else if (action === 'upload-folder') uploads.openFolderPicker();
		else void actions.handleContextMenuAction(action, []);
	}
	const selectedBytes = $derived(selectedFileBytes(data.displayItems, selection.paths));
	// Uploads keep running without their panel, so closing the error only hides the message.
	let uploadErrorDismissed = $state(false);
	let launchTabId = $state('');
	let detailsOpen = $state(false);
	let shortcutsOpen = $state(false);
	let toolbarRef = $state<ReturnType<typeof ToolbarComponent>>();

	const tabs = useBrowseTabs({
		initialLocation: launchState.location,
		onActiveLocationChange(snapshot) {
			if (pendingSelectPath && tabs.activeTabId !== launchTabId) pendingSelectPath = null;
			syncBrowseUrl(snapshot, pendingSelectPath);
		}
	});
	launchTabId = tabs.activeTabId;

	const activePane = $derived(tabs.activePane);
	const activeLocation = $derived(activePane?.location);
	const activeRuntime = $derived(activePane ? (paneRuntimes[activePane.id] ?? null) : null);
	const settings = $derived(settingsStore.current);

	function closeDetails(): void {
		detailsOpen = false;
		void tick().then(() => toolbarRef?.focusDetailsToggle());
	}

	afterNavigate(({ type }) => {
		if (type === 'popstate') {
			untrack(() => activeLocation?.restore(parseBrowseLocation(page.url)));
		}
	});

	const onFolderHover = createHoverIntent((path) => {
		if (path) activeRuntime?.data.prefetchDirectory(path);
	}, CONFIG.query.prefetchHoverDelayMs);

	$effect(() => {
		void activePane?.id;
		void activeLocation?.path;
		return () => onFolderHover(null);
	});

	function syncBrowseUrl(snapshot: BrowseLocationSnapshot, selectPath: string | null): void {
		const url = new URL(page.url.href);
		if (snapshot.path) url.searchParams.set('path', snapshot.path);
		else url.searchParams.delete('path');
		if (snapshot.searchQuery) url.searchParams.set('q', snapshot.searchQuery);
		else url.searchParams.delete('q');
		url.searchParams.set('sort', snapshot.sortBy);
		url.searchParams.set('dir', snapshot.sortDir);
		url.searchParams.set('view', snapshot.viewMode);
		if (selectPath) url.searchParams.set('select', selectPath);
		else url.searchParams.delete('select');
		if (url.href === page.url.href) return;

		void goto(`${resolve('browse')}${url.search}${url.hash}`, {
			replace: page.url.searchParams.get('path') === snapshot.path,
			reset: false,
			state: page.state
		});
	}

	function setPaneRuntime(paneId: string, runtime: BrowsePaneRuntime | null): void {
		if (runtime) {
			paneRuntimes = { ...paneRuntimes, [paneId]: runtime };
			return;
		}

		if (!(paneId in paneRuntimes)) return;
		const next = { ...paneRuntimes };
		delete next[paneId];
		paneRuntimes = next;
	}

	function handleInitialSelectHandled(path: string): void {
		if (untrack(() => pendingSelectPath) !== path) return;
		pendingSelectPath = null;
		if (activePane) syncBrowseUrl(activePane.location.snapshot, null);
	}

	function focusActivePane(): void {
		const paneId = tabs.activePane?.id;
		if (!paneId) return;
		void tick().then(() => document.getElementById(`browse-pane-${paneId}`)?.focus());
	}

	function handleKeydown(event: KeyboardEvent): void {
		if (event.defaultPrevented || shortcutsOpen) return;
		if (event.key === 'F1' && !activeRuntime?.hasOpenDialog) {
			event.preventDefault();
			shortcutsOpen = true;
			return;
		}
		const workspace = document.getElementById('browse-workspace');
		const insideWorkspace =
			event.target instanceof Node && workspace?.contains(event.target) === true;
		if (!insideWorkspace && event.target !== document.body && !activeRuntime?.hasOpenDialog) return;

		const shortcut = getBrowseShortcut({
			key: event.key,
			ctrlKey: event.ctrlKey,
			metaKey: event.metaKey,
			shiftKey: event.shiftKey,
			altKey: event.altKey,
			editable: isTextEntryTarget(event.target)
		});
		if (!shortcut) return;
		event.preventDefault();
		if (activeRuntime?.hasOpenDialog) return;

		switch (shortcut) {
			case 'new-tab':
				tabs.newTab();
				focusActivePane();
				break;
			case 'close-tab':
				tabs.closeTab();
				focusActivePane();
				break;
			case 'next-tab':
				tabs.cycleTab(1);
				focusActivePane();
				break;
			case 'previous-tab':
				tabs.cycleTab(-1);
				focusActivePane();
				break;
			case 'toggle-split':
				tabs.toggleSplit();
				focusActivePane();
				break;
			case 'back':
				activeLocation?.back();
				break;
			case 'forward':
				activeLocation?.forward();
				break;
			case 'up':
				activeLocation?.up();
				break;
			case 'refresh':
				activeRuntime?.data.reload();
				break;
			case 'toggle-hidden':
				settingsStore.toggleHiddenFiles();
				break;
			case 'focus-location':
				toolbarRef?.focusLocation();
				break;
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<svelte:head>
	<title>BoxBox</title>
</svelte:head>

<div class="browse-shell flex h-screen w-full overflow-hidden bg-surface-primary">
	<!-- Sidebar -->
	<Sidebar
		currentPath={activeLocation?.path ?? ''}
		roots={activeRuntime?.data.roots ?? []}
		onNavigate={(path) => activeLocation?.navigate(path)}
		{onFolderHover}
		open={sidebarOpen}
		onClose={() => (sidebarOpen = false)}
	/>

	<!-- Main content area -->
	<div class="flex min-h-0 min-w-0 flex-1 flex-col">
		<!-- Toolbar with navigation and path bar -->
		<Toolbar
			pathSegments={location.segments}
			canGoBack={location.canGoBack}
			canGoForward={location.canGoForward}
			onBack={location.back}
			onForward={location.forward}
			onNavigate={location.navigate}
			onRefresh={data.reload}
			onSettings={() => (settingsOpen = true)}
			onToggleSidebar={() => (sidebarOpen = !sidebarOpen)}
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

{#if actions.previewFile}
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

{#if settingsOpen}
	{#await import('#lib/components/settings/SettingsModal.svelte') then { default: SettingsModal }}
		<SettingsModal onclose={() => (settingsOpen = false)} />
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
<Notifications>
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
</Notifications>
