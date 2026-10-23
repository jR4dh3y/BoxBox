<script lang="ts">
	import type { FileInfo } from '#lib/api/files.js';
	import type { SortField, SortDir } from '#lib/types/files.js';
	import { formatFileSize, formatFileDate } from '#lib/utils/format.js';
	import { getFileTypeDescription, getFileIcon } from '#lib/utils/fileTypes.js';
	import { SvelteSet } from 'svelte/reactivity';
	import { untrack } from 'svelte';
	import Spinner from '#lib/components/ui/Spinner.svelte';
	import LoadError from '#lib/components/LoadError.svelte';
	import { FolderOpen } from 'lucide-svelte';
	import { CONFIG } from '#lib/config.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';
	import FileThumbnail from '#lib/components/FileThumbnail.svelte';
	import { useFileCollection, fileDisplayName } from '#lib/browse/fileCollection.svelte.js';
	import type { FileContextAction } from '#lib/utils/fileContextMenu.js';
	import { supportsThumbnailFile } from '#lib/utils/thumbnail.js';
	import type { FileOpenBehavior } from '#lib/utils/fileOpenBehavior.js';
	import { createVirtualScroll } from '#lib/browse/virtualScroll.svelte.js';

	let {
		items = [],
		sortBy = 'name',
		sortDir = 'asc',
		emptyMessage = 'This folder is empty',
		selectedPaths = new SvelteSet<string>(),
		isLoading = false,
		compactMode = false,
		cutPaths = new SvelteSet<string>(),
		favoritePaths = new SvelteSet<string>(),
		canPaste = false,
		canCreate = false,
		showFileExtensions = true,
		fileOpenBehavior = 'double-click',
		revealPath = null,
		onItemClick,
		onFolderHover,
		onSortChange,
		onSelectionChange,
		onContextMenuAction
	}: {
		items?: FileInfo[];
		sortBy?: SortField;
		sortDir?: SortDir;
		emptyMessage?: string;
		selectedPaths?: Set<string>;
		isLoading?: boolean;
		compactMode?: boolean;
		cutPaths?: Set<string>;
		favoritePaths?: Set<string>;
		canPaste?: boolean;
		canCreate?: boolean;
		showFileExtensions?: boolean;
		fileOpenBehavior?: FileOpenBehavior;
		/** Scrolled into view once it appears in `items`. */
		revealPath?: string | null;
		onItemClick?: (item: FileInfo) => void;
		/** Path of the folder under the pointer, or null once it leaves */
		onFolderHover?: (path: string | null) => void;
		onSortChange?: (field: SortField, dir: SortDir) => void;
		onSelectionChange?: (paths: Set<string>) => void;
		onContextMenuAction?: (action: FileContextAction, items: FileInfo[]) => void;
	} = $props();

	const showSpinner = createDelayedFlag(() => isLoading, CONFIG.ui.spinnerDelayMs);
	let headerHeight = $state(0);
	const virtual = createVirtualScroll(() => ({
		count: items.length,
		rowGap: 0,
		offsetTop: headerHeight,
		stickyTop: headerHeight,
		estimatedItemHeight: 33
	}));
	const visibleItems = $derived(items.slice(virtual.range.start, virtual.range.end));

	// Bring an item the pane selected for the user into view once, when it is in the list.
	let revealedPath: string | null = null;
	$effect(() => {
		const path = revealPath;
		if (!path || path === revealedPath || !virtual.ready) return;
		const index = items.findIndex((item) => item.path === path);
		if (index < 0) return;
		revealedPath = path;
		untrack(() => virtual.reveal(index));
	});
	const collection = useFileCollection(
		() => ({
			items,
			selectedPaths,
			fileOpenBehavior,
			onItemClick,
			onSelectionChange,
			onContextMenuAction
		}),
		virtual
	);

	function handleSort(field: SortField) {
		onSortChange?.(field, sortBy === field && sortDir === 'asc' ? 'desc' : 'asc');
	}

	function getSortIndicator(field: SortField): string {
		if (sortBy !== field) return '';
		return sortDir === 'asc' ? '▲' : '▼';
	}

	const thClass =
		'text-left px-3 py-2 bg-surface-secondary border-b border-border-primary font-medium text-text-secondary whitespace-nowrap select-none sticky top-0 z-[5] cursor-pointer transition-colors duration-100 hover:bg-surface-tertiary hover:text-text-primary focus:outline focus:outline-1 focus:outline-accent focus:-outline-offset-1';
	const thSortedClass = 'text-accent';
	const tdClass = 'h-8 px-3 py-1.5 align-middle border-b border-border-secondary text-text-primary';
	const clippedCellClass = 'block overflow-hidden text-ellipsis whitespace-nowrap';
	const selectionRevealClass =
		'flex items-center justify-center group-hover/row:invisible group-has-[input:focus-visible]/row:invisible';
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="file-list-container relative h-full w-full overflow-auto bg-surface-primary outline-none {compactMode
		? 'compact'
		: ''}"
	tabindex="-1"
	{@attach virtual.scroller}
	onkeydown={collection.scrollerKeydown}
	oncontextmenu={(event) =>
		collection.backgroundContextMenu(event, '[data-file-row="true"], [data-file-header="true"]')}
>
	{#if showSpinner.value}
		<div class="absolute inset-0 z-10 flex items-center justify-center bg-surface-primary/80">
			<Spinner />
		</div>
	{/if}

	<table
		class="w-full table-fixed border-collapse text-[13px] leading-5"
		role="grid"
		aria-busy={isLoading}
		aria-multiselectable="true"
		aria-rowcount={items.length + 1}
	>
		<colgroup>
			<col class="file-name-column" />
			<col class="file-type-column" />
			<col class="file-size-column" />
			<col class="file-modified-column" />
		</colgroup>
		<thead bind:offsetHeight={headerHeight}>
			<tr aria-rowindex={1}>
				<th
					class="{thClass} {sortBy === 'name' ? thSortedClass : ''}"
					onclick={() => handleSort('name')}
					onkeydown={(e) => e.key === 'Enter' && handleSort('name')}
					tabindex="0"
					role="columnheader"
					data-file-header="true"
					aria-sort={sortBy === 'name' ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
				>
					<span class="mr-1">Name</span>
					<span class="text-[10px] opacity-80">{getSortIndicator('name')}</span>
				</th>
				<th
					class="file-type-column {thClass} {sortBy === 'type' ? thSortedClass : ''}"
					onclick={() => handleSort('type')}
					onkeydown={(e) => e.key === 'Enter' && handleSort('type')}
					tabindex="0"
					role="columnheader"
					data-file-header="true"
					aria-sort={sortBy === 'type' ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
				>
					<span class="mr-1">Type</span>
					<span class="text-[10px] opacity-80">{getSortIndicator('type')}</span>
				</th>
				<th
					class="file-size-column {thClass} text-right {sortBy === 'size' ? thSortedClass : ''}"
					onclick={() => handleSort('size')}
					onkeydown={(e) => e.key === 'Enter' && handleSort('size')}
					tabindex="0"
					role="columnheader"
					data-file-header="true"
					aria-sort={sortBy === 'size' ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
				>
					<span class="mr-1">Size</span>
					<span class="text-[10px] opacity-80">{getSortIndicator('size')}</span>
				</th>
				<th
					class="file-modified-column {thClass} {sortBy === 'modTime' ? thSortedClass : ''}"
					onclick={() => handleSort('modTime')}
					onkeydown={(e) => e.key === 'Enter' && handleSort('modTime')}
					tabindex="0"
					role="columnheader"
					data-file-header="true"
					aria-sort={sortBy === 'modTime'
						? sortDir === 'asc'
							? 'ascending'
							: 'descending'
						: 'none'}
				>
					<span class="mr-1">Modified</span>
					<span class="text-[10px] opacity-80">{getSortIndicator('modTime')}</span>
				</th>
			</tr>
		</thead>
		<tbody>
			{#if items.length === 0 && !isLoading}
				<tr>
					<td colspan="4" class="px-3 py-12">
						<div class="flex flex-col items-center gap-2 text-text-muted">
							<FolderOpen size={32} class="opacity-50" />
							<span class="text-[13px]">{emptyMessage}</span>
						</div>
					</td>
				</tr>
			{:else}
				<tr aria-hidden="true" style:height="{virtual.range.before}px"
					><td colspan="4" class="p-0"></td></tr
				>
				{#each visibleItems as item, offset (item.path)}
					{@const index = virtual.range.start + offset}
					{@const IconComponent = getFileIcon(item.name, item.isDir)}
					{@const displayName = fileDisplayName(item, showFileExtensions)}
					{@const typeDescription = item.isDir ? 'Folder' : getFileTypeDescription(item.name)}
					{@const sizeDescription = item.isDir ? '' : formatFileSize(item.size)}
					{@const modifiedDescription = formatFileDate(item.modTime)}
					{@const hasThumbnail = supportsThumbnailFile(item)}
					<tr
						class="file-list-row group/row cursor-default transition-colors duration-50 focus:bg-selection focus:outline-none {selectedPaths.has(
							item.path
						)
							? 'bg-selection hover:bg-selection-hover'
							: 'hover:bg-surface-secondary'} {cutPaths.has(item.path) ? 'opacity-50' : ''}"
						onclick={(event) => collection.click(item, event)}
						onpointerenter={() => item.isDir && onFolderHover?.(item.path)}
						onpointerleave={() => onFolderHover?.(null)}
						onkeydown={(event) => collection.keydown(item, event)}
						ondblclick={(event) => collection.doubleClick(item, event)}
						oncontextmenu={(event) => collection.itemContextMenu(item, event)}
						tabindex="0"
						aria-selected={selectedPaths.has(item.path)}
						aria-rowindex={index + 2}
						data-file-row="true"
						data-file-path={item.path}
						data-file-index={index}
						{@attach virtual.item}
					>
						<td class={tdClass}>
							<div class="flex min-w-0 items-center gap-2">
								<!-- The checkbox takes the icon's place while the row is pointed at or focused. -->
								<span
									class="relative flex h-5 w-5 shrink-0 items-center justify-center {item.isDir
										? 'text-folder'
										: 'text-text-secondary'}"
								>
									<span class="{selectionRevealClass} pointer-coarse:invisible">
										{#if hasThumbnail}
											<span class="block h-5 w-5 overflow-hidden rounded-sm">
												<FileThumbnail file={item} alt="" fit="cover" class="h-full" />
											</span>
										{:else}
											<IconComponent size={16} />
										{/if}
									</span>
									<input
										type="checkbox"
										checked={selectedPaths.has(item.path)}
										aria-label={`Select ${item.name}`}
										class="peer absolute inset-0.5 m-0 cursor-pointer accent-accent opacity-0 group-hover/row:opacity-100 focus-visible:opacity-100 pointer-coarse:opacity-100"
										onclick={(event) => event.stopPropagation()}
										ondblclick={(event) => event.stopPropagation()}
										onchange={(event) => collection.check(item, event.currentTarget.checked)}
									/>
								</span>
								<span
									class="{clippedCellClass} min-w-0 flex-1 {item.isDir ? 'text-folder' : ''}"
									title={item.name}
								>
									{displayName}
								</span>
							</div>
						</td>
						<td class="file-type-column {tdClass} text-text-secondary">
							<span class={clippedCellClass} title={typeDescription}>{typeDescription}</span>
						</td>
						<td class="file-size-column {tdClass} text-right text-text-secondary tabular-nums">
							<span class={clippedCellClass} title={sizeDescription}>{sizeDescription}</span>
						</td>
						<td class="file-modified-column {tdClass} text-text-secondary">
							<span class={clippedCellClass} title={modifiedDescription}>{modifiedDescription}</span
							>
						</td>
					</tr>
				{/each}
				<tr aria-hidden="true" style:height="{virtual.range.after}px"
					><td colspan="4" class="p-0"></td></tr
				>
			{/if}
		</tbody>
	</table>
</div>

{#if collection.contextMenu}
	{@const contextMenu = collection.contextMenu}
	{#await import('#lib/components/FileContextMenu.svelte') then { default: FileContextMenu }}
		<FileContextMenu
			items={contextMenu.items}
			{canPaste}
			{favoritePaths}
			{canCreate}
			x={contextMenu.x}
			y={contextMenu.y}
			onSelect={collection.selectMenuAction}
			onClose={collection.closeMenu}
		/>
	{:catch}
		<LoadError label="file menu" onClose={collection.closeMenu} />
	{/await}
{/if}

<style>
	.file-list-container {
		container-name: file-list;
		container-type: inline-size;
	}

	.file-type-column {
		width: 22%;
	}

	.file-size-column {
		width: 14%;
	}

	.file-modified-column {
		width: 24%;
	}

	@container file-list (max-width: 600px) {
		.file-modified-column {
			display: none;
		}
	}

	@container file-list (max-width: 460px) {
		.file-type-column {
			display: none;
		}
	}
</style>
