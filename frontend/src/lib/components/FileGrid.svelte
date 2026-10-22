<script lang="ts">
	/**
	 * FileGrid component - icon/thumbnail grid for browsing folders
	 */

	import type { FileInfo } from '#lib/api/files.js';
	import FileThumbnail from '#lib/components/FileThumbnail.svelte';
	import { supportsThumbnailFile } from '#lib/utils/thumbnail.js';
	import { formatFileSize, formatFileDate } from '#lib/utils/format.js';
	import { getFileTypeDescription, getFileIcon } from '#lib/utils/fileTypes.js';
	import { getFileContextMenuItems } from '#lib/utils/fileContextMenu.js';
	import { SvelteSet } from 'svelte/reactivity';
	import { ContextMenu, Spinner } from '#lib/components/ui/index.js';
	import { FolderOpen } from 'lucide-svelte';
	import { CONFIG } from '#lib/config.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';
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
		fileOpenBehavior?: FileOpenBehavior;
		onItemClick?: (item: FileInfo) => void;
		/** Path of the folder under the pointer, or null once it leaves */
		onFolderHover?: (path: string | null) => void;
		onSelectionChange?: (paths: Set<string>) => void;
		onContextMenuAction?: (action: string, items: FileInfo[]) => void;
	} = $props();

	const showSpinner = createDelayedFlag(() => isLoading, CONFIG.ui.spinnerDelayMs);
	let selectionAnchor: string | null = null;

	let contextMenu = $state<{ x: number; y: number; items: FileInfo[] } | null>(null);

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

	function handleItemClick(item: FileInfo, event: MouseEvent) {
		updateSelection(item.path, fileSelectionGestureFromModifiers(event));
		if (shouldOpenOnClick(fileOpenBehavior, event)) onItemClick?.(item);
	}

	function handleDoubleClick(item: FileInfo, event: MouseEvent) {
		if (shouldOpenOnDoubleClick(fileOpenBehavior, event)) onItemClick?.(item);
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
		if (target?.closest('[data-file-item="true"]')) return;

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
		'group relative flex aspect-[1/1.15] min-w-0 cursor-default flex-col rounded border border-transparent bg-transparent p-2 text-left outline-none transition-colors duration-100 hover:bg-surface-secondary focus:border-border-focus focus:bg-surface-secondary';
	const selectedClass = 'border-accent/70 bg-selection hover:bg-selection-hover';
	const thumbnailClass =
		'flex shrink-0 items-center justify-center overflow-hidden rounded border border-border-primary bg-surface-primary';
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="relative h-full w-full overflow-auto bg-surface-primary"
	oncontextmenu={handleBackgroundContextMenu}
>
	{#if showSpinner.value}
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
				{@const showThumbnail = supportsThumbnailFile(item)}
				{@const detailText = item.isDir ? 'Folder' : sizeDescription}
				<div
					class="file-grid-item {tileClass} {isSelected(item.path) ? selectedClass : ''} {isCut(
						item.path
					)
						? 'opacity-50'
						: ''}"
					role="gridcell"
					tabindex="0"
					aria-selected={isSelected(item.path)}
					title={`${item.name}\n${typeDescription}${sizeDescription ? ` - ${sizeDescription}` : ''}\nModified ${modifiedDescription}`}
					onclick={(e) => handleItemClick(item, e)}
					onpointerenter={() => item.isDir && onFolderHover?.(item.path)}
					onpointerleave={() => onFolderHover?.(null)}
					ondblclick={(event) => handleDoubleClick(item, event)}
					onkeydown={(e) => handleKeyDown(item, e)}
					oncontextmenu={(e) => handleContextMenu(item, e)}
					data-file-item="true"
				>
					<input
						type="checkbox"
						checked={isSelected(item.path)}
						aria-label={`Select ${item.name}`}
						class="absolute top-2 left-2 z-[1] h-4 w-4 cursor-pointer accent-accent"
						onclick={(event) => event.stopPropagation()}
						ondblclick={(event) => event.stopPropagation()}
						onchange={(event) => handleSelectionCheckbox(item, event.currentTarget.checked)}
					/>
					<div class="{thumbnailClass} {compactMode ? 'h-11' : 'h-16'}">
						{#if showThumbnail}
							<FileThumbnail
								file={item}
								width={compactMode ? 192 : 224}
								height={compactMode ? 96 : 144}
								alt=""
								class="h-full"
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

	.file-grid-item {
		content-visibility: auto;
		contain-intrinsic-size: 112px 129px;
	}
</style>
