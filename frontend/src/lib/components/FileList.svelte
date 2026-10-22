<script lang="ts">
	import type { FileInfo } from '#lib/api/files.js';
	import type { SortField, SortDir } from '#lib/types/files.js';
	import { formatFileSize, formatFileDate } from '#lib/utils/format.js';
	import { getFileTypeDescription, getFileIcon } from '#lib/utils/fileTypes.js';
	import { SvelteSet } from 'svelte/reactivity';
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
		onItemClick?: (item: FileInfo) => void;
		/** Path of the folder under the pointer, or null once it leaves */
		onFolderHover?: (path: string | null) => void;
		onSortChange?: (field: SortField, dir: SortDir) => void;
		onSelectionChange?: (paths: Set<string>) => void;
		onContextMenuAction?: (action: FileContextAction, items: FileInfo[]) => void;
	} = $props();

	const showSpinner = createDelayedFlag(() => isLoading, CONFIG.ui.spinnerDelayMs);
	const collection = useFileCollection(
		() => ({
			items,
			selectedPaths,
			fileOpenBehavior,
			onItemClick,
			onSelectionChange,
			onContextMenuAction
		}),
		'list'
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
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="file-list-container relative h-full w-full overflow-auto bg-surface-primary {compactMode
		? 'compact'
		: ''}"
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
	>
		<colgroup>
			<col class="file-name-column" />
			<col class="file-type-column" />
			<col class="file-size-column" />
			<col class="file-modified-column" />
		</colgroup>
		<thead>
			<tr>
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
				{#each items as item (item.path)}
					{@const IconComponent = getFileIcon(item.name, item.isDir)}
					{@const displayName = fileDisplayName(item, showFileExtensions)}
					{@const typeDescription = item.isDir ? 'Folder' : getFileTypeDescription(item.name)}
					{@const sizeDescription = item.isDir ? '' : formatFileSize(item.size)}
					{@const modifiedDescription = formatFileDate(item.modTime)}
					{@const hasThumbnail = supportsThumbnailFile(item)}
					<tr
						class="file-list-row cursor-default transition-colors duration-50 hover:bg-surface-secondary focus:bg-selection focus:outline-none {selectedPaths.has(
							item.path
						)
							? 'bg-selection hover:bg-selection-hover'
							: ''} {cutPaths.has(item.path) ? 'opacity-50' : ''}"
						onclick={(event) => collection.click(item, event)}
						onpointerenter={() => item.isDir && onFolderHover?.(item.path)}
						onpointerleave={() => onFolderHover?.(null)}
						onkeydown={(event) => collection.keydown(item, event)}
						ondblclick={(event) => collection.doubleClick(item, event)}
						oncontextmenu={(event) => collection.itemContextMenu(item, event)}
						tabindex="0"
						aria-selected={selectedPaths.has(item.path)}
						data-file-row="true"
						data-file-path={item.path}
					>
						<td class={tdClass}>
							<div class="flex min-w-0 items-center gap-2">
								<input
									type="checkbox"
									checked={selectedPaths.has(item.path)}
									aria-label={`Select ${item.name}`}
									class="h-4 w-4 shrink-0 cursor-pointer accent-accent"
									onclick={(event) => event.stopPropagation()}
									ondblclick={(event) => event.stopPropagation()}
									onchange={(event) => collection.check(item, event.currentTarget.checked)}
								/>
								<span
									class="flex h-7 w-12 shrink-0 items-center justify-center overflow-hidden rounded {item.isDir
										? 'text-folder'
										: 'text-text-secondary'}"
								>
									{#if hasThumbnail}
										<FileThumbnail file={item} width={224} height={96} />
									{:else}
										<IconComponent size={16} />
									{/if}
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

	.file-list-row {
		content-visibility: auto;
		contain-intrinsic-size: 32px;
	}
</style>
