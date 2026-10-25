<script lang="ts">
	import type { BrowsePaneRuntime } from '#lib/browse/pane.svelte.js';
	import type { FileDropTarget } from '#lib/browse/fileDrag.js';
	import type { FileInfo } from '#lib/api/files.js';
	import type { FileContextAction } from '#lib/utils/fileContextMenu.js';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import FileList from './FileList.svelte';
	import LoadError from './LoadError.svelte';

	let {
		runtime,
		dropTarget,
		revealPath,
		onFolderHover,
		onContextMenuAction
	}: {
		runtime: BrowsePaneRuntime;
		dropTarget: FileDropTarget;
		revealPath: string | null;
		onFolderHover: (path: string | null) => void;
		onContextMenuAction: (action: FileContextAction, items: FileInfo[]) => void;
	} = $props();
	const location = $derived(runtime.pane.location);
	const settings = $derived(settingsStore.current);
	const viewProps = $derived({
		items: runtime.data.displayItems,
		path: location.path,
		viewKey: JSON.stringify([
			location.path,
			location.trimmedSearchQuery,
			location.sortBy,
			location.sortDir
		]),
		scrollPositions: runtime.pane.scrollPositions,
		emptyMessage: runtime.data.emptyListMessage,
		selectedPaths: runtime.pane.selection.paths,
		isLoading: runtime.data.isFileListLoading,
		compactMode: settings.compactMode,
		cutPaths: runtime.actions.cutPaths,
		favoritePaths: runtime.actions.favoritePaths,
		canPaste: runtime.actions.canPaste,
		canCreate: runtime.data.canCreate,
		showFileExtensions: settings.showFileExtensions,
		fileOpenBehavior: settings.fileOpenBehavior,
		dropTarget,
		revealPath,
		onItemClick: runtime.actions.openFile,
		onFolderHover,
		onSelectionChange: runtime.pane.selection.set,
		onContextMenuAction
	});
</script>

{#if location.viewMode === 'grid'}
	{#await import('./FileGrid.svelte') then { default: FileGrid }}
		<FileGrid {...viewProps} />
	{:catch}
		<LoadError label="grid view" inline />
	{/await}
{:else}
	<FileList
		{...viewProps}
		sortBy={location.sortBy}
		sortDir={location.sortDir}
		onSortChange={location.setSort}
	/>
{/if}
