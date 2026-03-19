<script lang="ts">
	/**
	 * FileList component with sortable columns - FilePilot style
	 * Requirements: 1.1, 1.2, Context Menu
	 */

	import type { FileInfo } from '$lib/api/files';
	import type { SortField, SortDir } from '$lib/types/files';
	import { formatFileSize, formatFileDate } from '$lib/utils/format';
	import { getFileTypeDescription, getFileIcon } from '$lib/utils/fileTypes';
	import { getFileContextMenuItems } from '$lib/utils/fileContextMenu';
	import { SvelteSet } from 'svelte/reactivity';
	import { Spinner, ContextMenu } from '$lib/components/ui';
	import { FolderOpen } from 'lucide-svelte';

	let {
		items = [],
		sortBy = 'name',
		sortDir = 'asc',
		emptyMessage = 'This folder is empty',
		selectedPaths = new SvelteSet<string>(),
		isLoading = false,
		compactMode = false,
		cutPaths = new SvelteSet<string>(),
		canPaste = false,
		onItemClick,
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
		canPaste?: boolean;
		onItemClick?: (item: FileInfo) => void;
		onSortChange?: (field: SortField, dir: SortDir) => void;
		onSelectionChange?: (paths: Set<string>) => void;
		onContextMenuAction?: (action: string, items: FileInfo[]) => void;
	} = $props();

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
		if (event.ctrlKey || event.metaKey) {
			const newSelection = new SvelteSet<string>(selectedPaths);
			if (newSelection.has(item.path)) {
				newSelection.delete(item.path);
			} else {
				newSelection.add(item.path);
			}
			onSelectionChange?.(newSelection);
		} else if (event.shiftKey && selectedPaths.size > 0) {
			const newSelection = new SvelteSet<string>(selectedPaths);
			newSelection.add(item.path);
			onSelectionChange?.(newSelection);
		} else {
			const newSelection = new SvelteSet<string>([item.path]);
			onSelectionChange?.(newSelection);
		}
	}

	function handleDoubleClick(item: FileInfo) {
		onItemClick?.(item);
	}

	function handleKeyDown(item: FileInfo, event: KeyboardEvent) {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			onItemClick?.(item);
		}
	}

	function handleContextMenu(item: FileInfo, event: MouseEvent) {
		event.preventDefault();

		// If right-clicked item is not selected, select only that item
		if (!selectedPaths.has(item.path)) {
			const newSelection = new SvelteSet<string>([item.path]);
			onSelectionChange?.(newSelection);
		}

		// Get all selected items for context menu
		const selectedItems = items.filter((i) => selectedPaths.has(i.path) || i.path === item.path);

		contextMenu = {
			x: event.clientX,
			y: event.clientY,
			items: selectedItems.length > 0 ? selectedItems : [item]
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

	const thClass =
		'text-left px-3 py-2 bg-surface-secondary border-b border-border-primary font-medium text-text-secondary whitespace-nowrap select-none sticky top-0 z-[5] cursor-pointer transition-colors duration-100 hover:bg-surface-tertiary hover:text-text-primary focus:outline focus:outline-1 focus:outline-accent focus:-outline-offset-1';
	const thSortedClass = 'text-accent';
	const tdClass = 'px-3 py-1.5 border-b border-border-secondary text-text-primary';
</script>

<div class="relative h-full w-full overflow-auto bg-surface-primary {compactMode ? 'compact' : ''}">
	{#if isLoading}
		<div class="absolute inset-0 z-10 flex items-center justify-center bg-surface-primary/80">
			<Spinner />
		</div>
	{/if}

	<table class="w-full border-collapse text-[13px]" role="grid" aria-busy={isLoading}>
		<thead>
			<tr>
				<th
					class="{thClass} min-w-[200px] {sortBy === 'name' ? thSortedClass : ''}"
					onclick={() => handleSort('name')}
					onkeydown={(e) => e.key === 'Enter' && handleSort('name')}
					tabindex="0"
					role="columnheader"
					aria-sort={sortBy === 'name' ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
				>
					<span class="mr-1">Name</span>
					<span class="text-[10px] opacity-80">{getSortIndicator('name')}</span>
				</th>
				<th
					class="{thClass} w-[120px] {sortBy === 'type' ? thSortedClass : ''}"
					onclick={() => handleSort('type')}
					onkeydown={(e) => e.key === 'Enter' && handleSort('type')}
					tabindex="0"
					role="columnheader"
					aria-sort={sortBy === 'type' ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
				>
					<span class="mr-1">Type</span>
					<span class="text-[10px] opacity-80">{getSortIndicator('type')}</span>
				</th>
				<th
					class="{thClass} w-[100px] text-right {sortBy === 'size' ? thSortedClass : ''}"
					onclick={() => handleSort('size')}
					onkeydown={(e) => e.key === 'Enter' && handleSort('size')}
					tabindex="0"
					role="columnheader"
					aria-sort={sortBy === 'size' ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
				>
					<span class="mr-1">Size</span>
					<span class="text-[10px] opacity-80">{getSortIndicator('size')}</span>
				</th>
				<th
					class="{thClass} w-[150px] {sortBy === 'modTime' ? thSortedClass : ''}"
					onclick={() => handleSort('modTime')}
					onkeydown={(e) => e.key === 'Enter' && handleSort('modTime')}
					tabindex="0"
					role="columnheader"
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
					<tr
						class="cursor-default transition-colors duration-50 hover:bg-surface-secondary focus:bg-selection focus:outline-none {isSelected(
							item.path
						)
							? 'bg-selection hover:bg-selection-hover'
							: ''} {isCut(item.path) ? 'opacity-50' : ''}"
						onclick={(e) => handleRowClick(item, e)}
						onkeydown={(e) => handleKeyDown(item, e)}
						ondblclick={() => handleDoubleClick(item)}
						oncontextmenu={(e) => handleContextMenu(item, e)}
						tabindex="0"
						aria-selected={isSelected(item.path)}
					>
						<td class="{tdClass} min-w-[200px]">
							<div class="flex items-center gap-2">
								<span
									class="flex w-5 shrink-0 items-center justify-center {item.isDir
										? 'text-folder'
										: 'text-text-secondary'}"
								>
									<IconComponent size={16} />
								</span>
								<span
									class="overflow-hidden text-ellipsis whitespace-nowrap {item.isDir
										? 'text-folder'
										: ''}"
									title={item.name}
								>
									{item.name}
								</span>
							</div>
						</td>
						<td class="{tdClass} w-[120px] text-text-secondary">
							{item.isDir ? 'Folder' : getFileTypeDescription(item.name)}
						</td>
						<td class="{tdClass} w-[100px] text-right text-text-secondary tabular-nums">
							{item.isDir ? '' : formatFileSize(item.size)}
						</td>
						<td class="{tdClass} w-[150px] whitespace-nowrap text-text-secondary">
							{formatFileDate(item.modTime)}
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
		items={getFileContextMenuItems({ items: contextMenu.items, canPaste })}
		x={contextMenu.x}
		y={contextMenu.y}
		onSelect={handleContextMenuSelect}
		onClose={handleContextMenuClose}
	/>
{/if}
