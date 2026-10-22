<script lang="ts">
	/**
	 * FileList component with sortable columns - FilePilot style
	 */

	import { moveFileFocus } from '#lib/browse/fileShortcuts.js';
	import type { FileInfo } from '#lib/api/files.js';
	import type { SortField, SortDir } from '#lib/types/files.js';
	import { formatFileSize, formatFileDate } from '#lib/utils/format.js';
	import { getFileTypeDescription, getFileIcon, getPreviewType } from '#lib/utils/fileTypes.js';
	import { getFileContextMenuItems } from '#lib/utils/fileContextMenu.js';
	import { SvelteSet } from 'svelte/reactivity';
	import { Spinner, ContextMenu } from '#lib/components/ui/index.js';
	import { FolderOpen } from 'lucide-svelte';
	import { CONFIG } from '#lib/config.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';
	import FileThumbnail from '#lib/components/FileThumbnail.svelte';
	import {
		applyFileSelection,
		fileSelectionGestureFromModifiers,
		getSelectedContextMenuItems,
		shouldOpenOnClick,
		shouldOpenOnDoubleClick,
		type FileSelectionGesture
	} from '#lib/browse/fileInteractions.js';
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
		onContextMenuAction?: (action: string, items: FileInfo[]) => void;
	} = $props();

	const showSpinner = createDelayedFlag(() => isLoading, CONFIG.ui.spinnerDelayMs);
	let selectionAnchor: string | null = null;

	// Context menu state
	let contextMenu = $state<{ x: number; y: number; items: FileInfo[] } | null>(null);

	function handleSort(field: SortField) {
		if (sortBy === field) {
			const newDir = sortDir === 'asc' ? 'desc' : 'asc';
			onSortChange?.(field, newDir);
		} else {
			onSortChange?.(field, 'asc');
		}
	}

	function handleRowClick(item: FileInfo, event: MouseEvent) {
		updateSelection(item.path, fileSelectionGestureFromModifiers(event));
		if (shouldOpenOnClick(fileOpenBehavior, event)) onItemClick?.(item);
	}

	function handleDoubleClick(item: FileInfo, event: MouseEvent) {
		if (shouldOpenOnDoubleClick(fileOpenBehavior, event)) onItemClick?.(item);
	}

	function updateSelection(itemPath: string, gesture: FileSelectionGesture): void {
		const result = applyFileSelection(
			items.map((item) => item.path),
			selectedPaths,
			selectionAnchor,
			itemPath,
			gesture
		);
		selectionAnchor = result.anchorPath;
		onSelectionChange?.(new SvelteSet(result.paths));
	}

	function handleSelectionCheckbox(item: FileInfo, selected: boolean): void {
		updateSelection(item.path, { type: 'set', selected });
	}

	function handleKeyDown(item: FileInfo, event: KeyboardEvent) {
		if ((event.key === 'a' || event.key === 'A') && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			selectionAnchor = item.path;
			onSelectionChange?.(new SvelteSet(items.map((visibleItem) => visibleItem.path)));
			return;
		}
		if (event.target instanceof HTMLInputElement) return;

		const nextPath = moveFileFocus(event, 'list');
		if (nextPath) {
			if (!event.ctrlKey && !event.metaKey) {
				selectionAnchor ??= item.path;
				updateSelection(nextPath, event.shiftKey ? { type: 'range' } : { type: 'replace' });
			}
			return;
		}
		if (event.altKey) return;

		if (event.key === 'Enter') {
			event.preventDefault();
			onItemClick?.(item);
			return;
		}

		if (event.key === ' ') {
			event.preventDefault();
			updateSelection(item.path, fileSelectionGestureFromModifiers(event));
			return;
		}

		// Match the shortcuts advertised in the context menu (F2, Del, Ctrl/Cmd+C/X/V)
		const targetItems = selectedPaths.has(item.path)
			? items.filter((selectedItem) => selectedPaths.has(selectedItem.path))
			: [item];

		if (event.key === 'Delete') {
			event.preventDefault();
			onContextMenuAction?.(event.shiftKey ? 'delete-permanently' : 'delete', targetItems);
		} else if (event.key === 'F2') {
			event.preventDefault();
			onContextMenuAction?.('rename', targetItems);
		} else if ((event.key === 'c' || event.key === 'C') && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			onContextMenuAction?.('copy', targetItems);
		} else if ((event.key === 'x' || event.key === 'X') && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			onContextMenuAction?.('cut', targetItems);
		} else if ((event.key === 'v' || event.key === 'V') && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			onContextMenuAction?.('paste', targetItems);
		}
	}

	function handleContextMenu(item: FileInfo, event: MouseEvent) {
		event.preventDefault();

		const selectedItems = getSelectedContextMenuItems(items, selectedPaths, item);
		if (!selectedPaths.has(item.path)) {
			const newSelection = new SvelteSet<string>([item.path]);
			selectionAnchor = item.path;
			onSelectionChange?.(newSelection);
		}

		contextMenu = {
			x: event.clientX,
			y: event.clientY,
			items: selectedItems
		};
	}

	function handleBackgroundContextMenu(event: MouseEvent) {
		const target = event.target instanceof HTMLElement ? event.target : null;
		if (target?.closest('[data-file-row="true"], [data-file-header="true"]')) return;

		event.preventDefault();
		selectionAnchor = null;
		onSelectionChange?.(new SvelteSet<string>());
		contextMenu = {
			x: event.clientX,
			y: event.clientY,
			items: []
		};
	}

	function handleContextMenuClose() {
		contextMenu = null;
	}

	function handleContextMenuSelect(action: string) {
		if (contextMenu && onContextMenuAction) {
			onContextMenuAction(action, contextMenu.items);
		}
		contextMenu = null;
	}

	function getSortIndicator(field: SortField): string {
		if (sortBy !== field) return '';
		return sortDir === 'asc' ? '▲' : '▼';
	}

	function isSelected(path: string): boolean {
		return selectedPaths.has(path);
	}

	function isCut(path: string): boolean {
		return cutPaths.has(path);
	}

	function getDisplayName(item: FileInfo): string {
		if (showFileExtensions || item.isDir) return item.name;

		const dotIndex = item.name.lastIndexOf('.');
		return dotIndex > 0 ? item.name.slice(0, dotIndex) : item.name;
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
	oncontextmenu={handleBackgroundContextMenu}
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
					{@const displayName = getDisplayName(item)}
					{@const typeDescription = item.isDir ? 'Folder' : getFileTypeDescription(item.name)}
					{@const sizeDescription = item.isDir ? '' : formatFileSize(item.size)}
					{@const modifiedDescription = formatFileDate(item.modTime)}
					{@const hasThumbnail = !item.isDir && getPreviewType(item.name) === 'image'}
					<tr
						class="file-list-row cursor-default transition-colors duration-50 hover:bg-surface-secondary focus:bg-selection focus:outline-none {isSelected(
							item.path
						)
							? 'bg-selection hover:bg-selection-hover'
							: ''} {isCut(item.path) ? 'opacity-50' : ''}"
						onclick={(e) => handleRowClick(item, e)}
						onpointerenter={() => item.isDir && onFolderHover?.(item.path)}
						onpointerleave={() => onFolderHover?.(null)}
						onkeydown={(e) => handleKeyDown(item, e)}
						ondblclick={(event) => handleDoubleClick(item, event)}
						oncontextmenu={(e) => handleContextMenu(item, e)}
						tabindex="0"
						aria-selected={isSelected(item.path)}
						data-file-row="true"
						data-file-path={item.path}
					>
						<td class={tdClass}>
							<div class="flex min-w-0 items-center gap-2">
								<input
									type="checkbox"
									checked={isSelected(item.path)}
									aria-label={`Select ${item.name}`}
									class="h-4 w-4 shrink-0 cursor-pointer accent-accent"
									onclick={(event) => event.stopPropagation()}
									ondblclick={(event) => event.stopPropagation()}
									onchange={(event) => handleSelectionCheckbox(item, event.currentTarget.checked)}
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

<!-- Context Menu -->
{#if contextMenu}
	<ContextMenu
		items={getFileContextMenuItems({
			items: contextMenu.items,
			canPaste,
			favoritePaths,
			canCreate
		})}
		x={contextMenu.x}
		y={contextMenu.y}
		onSelect={handleContextMenuSelect}
		onClose={handleContextMenuClose}
	/>
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
