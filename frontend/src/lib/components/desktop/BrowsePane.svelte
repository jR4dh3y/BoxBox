<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { useBrowseActions } from '#lib/browse/actions.svelte.js';
	import { useBrowseData } from '#lib/browse/data.svelte.js';
	import { useBrowseUploads } from '#lib/browse/uploads.svelte.js';
	import type { BrowsePaneState, BrowsePaneRuntime } from '#lib/browse/pane.svelte.js';
	import { CONFIG } from '#lib/config.js';
	import { createHoverIntent } from '#lib/utils/hoverIntent.js';
	import FileList from '#lib/components/FileList.svelte';
	import StatusBar from '#lib/components/StatusBar.svelte';
	import DriveCard from '#lib/components/DriveCard.svelte';
	import LoadError from '#lib/components/LoadError.svelte';
	import { Spinner } from '#lib/components/ui/index.js';

	interface Props {
		pane: BrowsePaneState;
		focused: boolean;
		initialSelectPath?: string | null;
		onFocus: () => void;
		onRuntimeChange: (runtime: BrowsePaneRuntime | null) => void;
		onInitialSelectHandled: (path: string) => void;
	}

	let {
		pane,
		focused,
		initialSelectPath = null,
		onFocus,
		onRuntimeChange,
		onInitialSelectHandled
	}: Props = $props();

	// The keyed pane model is stable for this component's lifetime.
	const paneState = untrack(() => pane);
	const { location, selection } = paneState;
	const data = useBrowseData(location);
	const actions = useBrowseActions(location, data, selection);
	const uploads = useBrowseUploads(location, data);
	const settings = $derived(settingsStore.current);

	const hasOpenDialog = $derived(
		actions.createDialog.open ||
			actions.renameDialog.open ||
			actions.deleteDialog.open ||
			actions.propertiesDialog.open ||
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
		actions.closeProperties();
		actions.closeOpenWith();
	}
</script>

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
					<div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
						{#each data.drives as drive (drive.name)}
							<DriveCard {drive} onClick={() => location.navigate(drive.name)} {onFolderHover} />
						{/each}
					</div>
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
					previewOnSingleClick={settings.previewOnSingleClick}
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
			createDialog={actions.createDialog}
			renameDialog={actions.renameDialog}
			deleteDialog={actions.deleteDialog}
			propertiesDialog={actions.propertiesDialog}
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
			onCloseProperties={actions.closeProperties}
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
