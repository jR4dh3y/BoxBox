<script lang="ts">
	import {
		getFileShortcut,
		isActivatableTarget,
		isTextEntryTarget,
		type FileShortcut
	} from '#lib/browse/fileShortcuts.js';
	import type { FileContextAction } from '#lib/utils/fileContextMenu.js';
	import { onMount, untrack } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { useBrowseActions } from '#lib/browse/actions.svelte.js';
	import { useBrowseData } from '#lib/browse/data.svelte.js';
	import { useBrowseUploads } from '#lib/browse/uploads.svelte.js';
	import type { BrowsePaneState, BrowsePaneRuntime } from '#lib/browse/pane.svelte.js';
	import type { FileInfo } from '#lib/api/files.js';
	import { CONFIG } from '#lib/config.js';
	import { createHoverIntent } from '#lib/utils/hoverIntent.js';
	import FileList from '#lib/components/FileList.svelte';
	import StatusBar from '#lib/components/StatusBar.svelte';
	import LoadError from '#lib/components/LoadError.svelte';
	import Spinner from '#lib/components/ui/Spinner.svelte';

	interface Props {
		pane: BrowsePaneState;
		focused: boolean;
		initialSelectPath?: string | null;
		onFocus: () => void;
		onRuntimeChange: (runtime: BrowsePaneRuntime | null) => void;
		onInitialSelectHandled: (path: string) => void;
		onShowDetails: () => void;
		onFocusSearch: () => void;
	}

	let {
		pane,
		focused,
		initialSelectPath = null,
		onFocus,
		onRuntimeChange,
		onInitialSelectHandled,
		onShowDetails,
		onFocusSearch
	}: Props = $props();

	// The keyed pane model is stable for this component's lifetime.
	const paneState = untrack(() => pane);
	const { location, selection } = paneState;
	const data = useBrowseData(location, () => paneState.revealHiddenFiles);
	const actions = useBrowseActions(location, data, selection);
	const uploads = useBrowseUploads(location, data);
	const settings = $derived(settingsStore.current);

	const hasOpenDialog = $derived(
		actions.createDialog.open ||
			actions.renameDialog.open ||
			actions.deleteDialog.open ||
			actions.openWithFile !== null
	);
	const runtime: BrowsePaneRuntime = {
		pane: paneState,
		data,
		actions,
		uploads,
		get hasOpenDialog() {
			return hasOpenDialog || actions.paste.plan !== null;
		}
	};
	const onFolderHover = createHoverIntent(
		data.prefetchDirectory,
		CONFIG.query.prefetchHoverDelayMs
	);

	onMount(() => {
		onRuntimeChange(runtime);
		return () => onRuntimeChange(null);
	});

	$effect(() => {
		void location.path;
		return () => onFolderHover(null);
	});

	$effect(() => {
		const target = initialSelectPath;
		if (!target) return;

		const parentPath = target.slice(0, Math.max(0, target.lastIndexOf('/')));
		if (location.path !== parentPath) {
			untrack(() => onInitialSelectHandled(target));
			return;
		}
		if (target.split('/').at(-1)?.startsWith('.') && !paneState.revealHiddenFiles) {
			untrack(() => paneState.showHiddenFiles());
			return;
		}

		const match = data.displayItems.find((item) => item.path === target);
		if (match) {
			selection.set(new SvelteSet([match.path]));
			untrack(() => onInitialSelectHandled(target));
			return;
		}

		if (data.isFileListLoading || data.isLoadingMore) return;
		if (data.hasMoreItems) {
			untrack(() => data.loadMore());
			return;
		}

		untrack(() => onInitialSelectHandled(target));
	});

	function closeDialogs(): void {
		actions.closeCreate();
		actions.closeRename();
		actions.closeDelete();
		actions.closeOpenWith();
	}

	function contextAction(action: FileContextAction, items: FileInfo[]): void {
		if (action === 'properties') {
			selection.set(new SvelteSet(items.map((item) => item.path)));
			onShowDetails();
			return;
		}
		void actions.handleContextMenuAction(action, items);
	}

	const writeShortcuts = new Set<FileShortcut>([
		'cut',
		'paste',
		'rename',
		'delete',
		'delete-permanently'
	]);

	function handleFileKeydown(event: KeyboardEvent): void {
		if (!focused || event.defaultPrevented || runtime.hasOpenDialog || actions.previewFile) return;
		if (document.querySelector('[role="menu"], dialog[open]')) return;
		if (
			isTextEntryTarget(event.target) ||
			(event.target instanceof Element &&
				event.target.closest('dialog, [role="dialog"], [role="menu"]'))
		)
			return;
		const shortcut = getFileShortcut(event);
		if (!shortcut || (shortcut === 'open' && isActivatableTarget(event.target))) return;
		event.preventDefault();
		const items = data.displayItems.filter((item) => selection.paths.has(item.path));
		switch (shortcut) {
			case 'select-all':
				selection.set(new SvelteSet(data.displayItems.map((item) => item.path)));
				return;
			case 'clear-selection':
				selection.clear();
				return;
			case 'search':
				onFocusSearch();
				return;
			case 'details':
				onShowDetails();
				return;
			case 'open':
				if (items.length === 1) actions.openFile(items[0]);
				return;
			case 'new-folder':
				if (data.canCreate) contextAction(shortcut, []);
				return;
		}
		if (data.isReadOnly && writeShortcuts.has(shortcut)) return;
		if (shortcut === 'paste' || items.length > 0) contextAction(shortcut, items);
	}
</script>

<svelte:window onkeydown={handleFileKeydown} />

<section
	id="browse-pane-{pane.id}"
	class="flex min-h-0 min-w-0 flex-1 flex-col outline-none {focused
		? 'ring-1 ring-accent/40 ring-inset'
		: ''}"
	tabindex="-1"
	aria-label="File browser pane"
	onpointerdown={onFocus}
	onfocusin={onFocus}
>
	<div
		class="relative min-h-0 flex-1 overflow-auto"
		ondragover={uploads.handleDragOver}
		ondragleave={uploads.handleDragLeave}
		ondrop={uploads.handleDrop}
		role="region"
		aria-label="File browser content"
	>
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
			<div class="p-6">
				{#if data.isLoadingDrives}
					<div class="flex items-center gap-2 py-5 text-sm text-text-secondary">
						<Spinner size="sm" />
						<span>Loading drives...</span>
					</div>
				{:else if data.drives.length === 0}
					<div class="py-5 text-sm text-text-secondary">No configured storage found</div>
				{:else}
					{#await import('#lib/components/DriveCard.svelte') then { default: DriveCard }}
						<div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
							{#each data.drives as drive (drive.name)}
								<DriveCard {drive} onClick={() => location.navigate(drive.name)} {onFolderHover} />
							{/each}
						</div>
					{:catch}
						<LoadError label="drives" inline />
					{/await}
				{/if}
			</div>
		{:else if location.viewMode === 'grid'}
			{#await import('#lib/components/FileGrid.svelte') then { default: FileGrid }}
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
					fileOpenBehavior={settings.fileOpenBehavior}
					onItemClick={actions.openFile}
					{onFolderHover}
					onSelectionChange={selection.set}
					onContextMenuAction={contextAction}
				/>
			{:catch}
				<LoadError label="grid view" inline />
			{/await}
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
				fileOpenBehavior={settings.fileOpenBehavior}
				onItemClick={actions.openFile}
				{onFolderHover}
				onSortChange={location.setSort}
				onSelectionChange={selection.set}
				onContextMenuAction={contextAction}
			/>
		{/if}
	</div>

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
</section>

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
			operation={actions.operation}
			createDialog={actions.createDialog}
			renameDialog={actions.renameDialog}
			deleteDialog={actions.deleteDialog}
			openWithFile={actions.openWithFile}
			onCloseOpenWith={actions.closeOpenWith}
			onCreateNameChange={actions.setCreateName}
			onRenameNameChange={actions.setRenameName}
			onCreateConfirm={() => void actions.confirmCreate()}
			onRenameConfirm={() => void actions.confirmRename()}
			onDeleteConfirm={() => void actions.confirmDelete()}
			onCloseCreate={actions.closeCreate}
			onCloseRename={actions.closeRename}
			onCloseDelete={actions.closeDelete}
		/>
	{:catch}
		<LoadError label="dialog" onClose={closeDialogs} />
	{/await}
{/if}

{#if actions.paste.plan}
	{#await import('./TransferConflictDialog.svelte') then { default: TransferConflictDialog }}
		<TransferConflictDialog
			open
			plan={actions.paste.plan}
			onapply={actions.paste.apply}
			oncancelbatch={() => void actions.paste.cancel()}
		/>
	{:catch}
		<LoadError label="transfer conflict dialog" onClose={() => void actions.paste.cancel()} />
	{/await}
{/if}

{#if actions.shareItem}
	{#await import('#lib/components/ShareModal.svelte') then { default: ShareModal }}
		<ShareModal open item={actions.shareItem} onclose={actions.closeShare} />
	{:catch}
		<LoadError label="share window" onClose={actions.closeShare} />
	{/await}
{/if}

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
