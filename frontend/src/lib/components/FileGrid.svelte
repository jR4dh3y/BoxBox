<script lang="ts">
	/**
	 * FileGrid component - icon/thumbnail grid for browsing folders
	 */

	import type { FileInfo } from '$lib/api/files';
	import { getPreviewUrl } from '$lib/api/files';
	import { formatFileSize, formatFileDate } from '$lib/utils/format';
	import { getFileTypeDescription, getFileIcon, getPreviewType } from '$lib/utils/fileTypes';
	import { getFileContextMenuItems } from '$lib/utils/fileContextMenu';
	import { SvelteSet } from 'svelte/reactivity';
	import { ContextMenu, Spinner } from '$lib/components/ui';
	import { FolderOpen } from 'lucide-svelte';

	let {
		items = [],
		emptyMessage = 'This folder is empty',
		selectedPaths = new SvelteSet<string>(),
		isLoading = false,
		compactMode = false,
		cutPaths = new SvelteSet<string>(),
		favoritePaths = new SvelteSet<string>(),
		canPaste = false,
		canCreate = false,
		showFileExtensions = true,
		previewOnSingleClick = false,
		onItemClick,
		onSelectionChange,
		onContextMenuAction
	}: {
		items?: FileInfo[];
		emptyMessage?: string;
		selectedPaths?: Set<string>;
		isLoading?: boolean;
		compactMode?: boolean;
		cutPaths?: Set<string>;
		favoritePaths?: Set<string>;
		canPaste?: boolean;
		canCreate?: boolean;
		showFileExtensions?: boolean;
		previewOnSingleClick?: boolean;
		onItemClick?: (item: FileInfo) => void;
		onSelectionChange?: (paths: Set<string>) => void;
		onContextMenuAction?: (action: string, items: FileInfo[]) => void;
	} = $props();

	let contextMenu = $state<{ x: number; y: number; items: FileInfo[] } | null>(null);
	let thumbnailErrors = new SvelteSet<string>();

	function handleItemClick(item: FileInfo, event: MouseEvent) {
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

		if (
			previewOnSingleClick &&
			!item.isDir &&
			!event.ctrlKey &&
			!event.metaKey &&
			!event.shiftKey
		) {
			onItemClick?.(item);
		}
	}

	function handleOpen(item: FileInfo) {
		if (previewOnSingleClick && !item.isDir) return;
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

		if (!selectedPaths.has(item.path)) {
			const newSelection = new SvelteSet<string>([item.path]);
			onSelectionChange?.(newSelection);
		}

		const selectedItems = items.filter((i) => selectedPaths.has(i.path) || i.path === item.path);

		contextMenu = {
			x: event.clientX,
			y: event.clientY,
			items: selectedItems.length > 0 ? selectedItems : [item]
		};
	}

	function handleBackgroundContextMenu(event: MouseEvent) {
		const target = event.target instanceof HTMLElement ? event.target : null;
		if (target?.closest('[data-file-item="true"]')) return;

		event.preventDefault();
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

	function handleThumbnailError(path: string) {
		thumbnailErrors.add(path);
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

	const gridClass = 'grid grid-cols-[repeat(auto-fill,112px)] justify-start gap-3 p-3';
	const compactGridClass = 'grid grid-cols-[repeat(auto-fill,96px)] justify-start gap-2 p-2';
	const tileClass =
		'group flex aspect-[1/1.15] min-w-0 cursor-default flex-col rounded border border-transparent bg-transparent p-2 text-left outline-none transition-colors duration-100 hover:bg-surface-secondary focus:border-border-focus focus:bg-surface-secondary';
	const selectedClass = 'border-accent/70 bg-selection hover:bg-selection-hover';
	const thumbnailClass =
		'flex shrink-0 items-center justify-center overflow-hidden rounded border border-border-primary bg-surface-primary';
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="relative h-full w-full overflow-auto bg-surface-primary"
	oncontextmenu={handleBackgroundContextMenu}
>
	{#if isLoading}
		<div class="absolute inset-0 z-10 flex items-center justify-center bg-surface-primary/80">
			<Spinner />
		</div>
	{/if}

	{#if items.length === 0 && !isLoading}
		<div class="flex h-full flex-col items-center justify-center gap-2 text-text-muted">
			<FolderOpen size={32} class="opacity-50" />
			<span class="text-[13px]">{emptyMessage}</span>
		</div>
	{:else}
		<div
			class={compactMode ? compactGridClass : gridClass}
			role="grid"
			aria-busy={isLoading}
			aria-multiselectable="true"
		>
			{#each items as item (item.path)}
				{@const IconComponent = getFileIcon(item.name, item.isDir)}
				{@const displayName = getDisplayName(item)}
				{@const typeDescription = item.isDir ? 'Folder' : getFileTypeDescription(item.name)}
				{@const sizeDescription = item.isDir ? '' : formatFileSize(item.size)}
				{@const modifiedDescription = formatFileDate(item.modTime)}
				{@const showThumbnail =
					!item.isDir && getPreviewType(item.name) === 'image' && !thumbnailErrors.has(item.path)}
				{@const detailText = item.isDir ? 'Folder' : sizeDescription}
				<div
					class="{tileClass} {isSelected(item.path) ? selectedClass : ''} {isCut(item.path)
						? 'opacity-50'
						: ''}"
					role="gridcell"
					tabindex="0"
					aria-selected={isSelected(item.path)}
					title={`${item.name}\n${typeDescription}${sizeDescription ? ` - ${sizeDescription}` : ''}\nModified ${modifiedDescription}`}
					onclick={(e) => handleItemClick(item, e)}
					ondblclick={() => handleOpen(item)}
					onkeydown={(e) => handleKeyDown(item, e)}
					oncontextmenu={(e) => handleContextMenu(item, e)}
					data-file-item="true"
				>
					<div class="{thumbnailClass} {compactMode ? 'h-11' : 'h-16'}">
						{#if showThumbnail}
							<img
								src={getPreviewUrl(item.path)}
								alt=""
								loading="lazy"
								class="h-full w-full object-cover"
								onerror={() => handleThumbnailError(item.path)}
							/>
						{:else}
							<span
								class="flex h-full w-full items-center justify-center {item.isDir
									? 'text-folder'
									: 'text-text-secondary'}"
							>
								<IconComponent size={compactMode ? 24 : 30} strokeWidth={1.7} />
							</span>
						{/if}
					</div>

					<div class="mt-1.5 min-w-0 overflow-hidden">
						<div class="file-grid-name text-center text-[13px] leading-4 text-text-primary">
							<span class={item.isDir ? 'text-folder' : ''}>{displayName}</span>
						</div>
						<div
							class="mt-0.5 truncate text-center text-[11px] leading-3.5 text-text-muted"
							title={typeDescription}
						>
							{detailText}
						</div>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

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
	.file-grid-name {
		display: -webkit-box;
		overflow: hidden;
		-webkit-box-orient: vertical;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		word-break: break-word;
	}
</style>
