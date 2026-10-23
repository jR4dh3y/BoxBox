<script lang="ts">
	import type { FileInfo } from '#lib/api/files.js';
	import type { SortField, SortDir } from '#lib/types/files.js';
	import { formatFileSize, formatFileDate } from '#lib/utils/format.js';
	import { getFileTypeDescription, getFileIcon } from '#lib/utils/fileTypes.js';
	import { SvelteSet } from 'svelte/reactivity';
	import { untrack } from 'svelte';
	import { Spinner } from '#lib/components/ui/index.js';
	import LoadError from '#lib/components/LoadError.svelte';
	import { ChevronDown, ChevronUp, FolderOpen } from 'lucide-svelte';
	import { CONFIG } from '#lib/config.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';
	import { createFixedRowWindow } from '#lib/utils/fixedRowWindow.svelte.js';
	import { withPathSelected } from '#lib/browse/selection.svelte.js';

	let {
		items = [],
		viewKey,
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
		previewOnSingleClick = false,
		selectionCheckboxes = true,
		onItemClick,
		onFolderHover,
		onSortChange,
		onSelectionChange,
		onContextMenuAction
	}: {
		items?: FileInfo[];
		viewKey: string;
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
		previewOnSingleClick?: boolean;
		selectionCheckboxes?: boolean;
		onItemClick?: (item: FileInfo) => void;
		/** Path of the folder under the pointer, or null once it leaves */
		onFolderHover?: (path: string | null) => void;
		onSortChange?: (field: SortField, dir: SortDir) => void;
		onSelectionChange?: (paths: Set<string>) => void;
		onContextMenuAction?: (action: FileContextAction, items: FileInfo[]) => void;
	} = $props();

	const showSpinner = createDelayedFlag(() => isLoading, CONFIG.ui.spinnerDelayMs);
	const LIST_HEADER_HEIGHT = 32;
	const columns: { field: SortField; label: string; align: 'left' | 'right' }[] = [
		{ field: 'name', label: 'Name', align: 'left' },
		{ field: 'type', label: 'Type', align: 'left' },
		{ field: 'size', label: 'Size', align: 'right' },
		{ field: 'modTime', label: 'Modified', align: 'left' }
	];
	let tableElement = $state<HTMLTableElement | null>(null);
	let activeItemPath = $state<string | null>(null);
	const activeItemIndex = $derived.by(() =>
		activeItemPath === null ? -1 : items.findIndex((item) => item.path === activeItemPath)
	);
	const activeItem = $derived(activeItemIndex < 0 ? null : (items[activeItemIndex] ?? null));
	const rowWindow = createFixedRowWindow({
		items: () => items,
		itemKey: (item) => item.path,
		viewKey: () => viewKey,
		isReady: () => !isLoading,
		rowHeight: () => (compactMode ? 28 : 32),
		itemOffset: () => LIST_HEADER_HEIGHT,
		viewportOffset: () => LIST_HEADER_HEIGHT
	});
	const attachRowWindow = rowWindow.attach;
	$effect(() => {
		const path = activeItemPath;
		if (path === null || items.some((item) => item.path === path)) return;
		untrack(() => {
			activeItemPath = null;
		});
	});

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
		activeItemPath = item.path;
		focusTable();
		if (event.ctrlKey || event.metaKey) {
			onSelectionChange?.(
				withPathSelected(selectedPaths, item.path, !selectedPaths.has(item.path))
			);
		} else if (event.shiftKey && selectedPaths.size > 0) {
			handleCheck(item, true);
		} else {
			const newSelection = new SvelteSet<string>([item.path]);
			onSelectionChange?.(newSelection);
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
		if (
			event.target instanceof HTMLInputElement &&
			(event.target.type !== 'checkbox' || event.key === ' ' || event.key === 'Enter')
		)
			return;
		if ((event.key === 'a' || event.key === 'A') && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			selectionAnchor = item.path;
			onSelectionChange?.(new SvelteSet(items.map((visibleItem) => visibleItem.path)));
			return;
		}

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

	function handleCheck(item: FileInfo, checked: boolean) {
		activeItemPath = item.path;
		onSelectionChange?.(withPathSelected(selectedPaths, item.path, checked));
		// Keyboard navigation stays on the table instead of the checkbox.
		focusTable();
	}

	function handleDoubleClick(item: FileInfo) {
		if (previewOnSingleClick && !item.isDir) return;
		onItemClick?.(item);
	}

	function focusTable() {
		tableElement?.focus({ preventScroll: true });
	}

	function handleTableFocus() {
		if (items.length === 0 || activeItemPath !== null) return;
		const firstVisible = Math.min(items.length - 1, rowWindow.firstVisibleRow);
		activeItemPath = items[firstVisible]?.path ?? null;
	}

	function handleViewportScroll() {
		rowWindow.onScroll();
	}

	function getKeyboardTargetItems(): FileInfo[] {
		if (selectedPaths.size > 0) {
			return items.filter((item) => selectedPaths.has(item.path));
		}
		return activeItem ? [activeItem] : [];
	}

	function handleKeyboardContextMenu() {
		if (!tableElement) return;
		const targetItems = getKeyboardTargetItems();
		if (targetItems.length === 0) return;

		const activeIsVisible =
			activeItemIndex >= rowWindow.firstVisibleRow && activeItemIndex < rowWindow.lastVisibleRow;
		const activeCell = activeIsVisible
			? tableElement.querySelector<HTMLElement>(`#file-list-cell-${activeItemIndex}`)
			: null;
		const bounds =
			activeCell?.getBoundingClientRect() ??
			tableElement.parentElement?.getBoundingClientRect() ??
			tableElement.getBoundingClientRect();
		contextMenu = {
			x: Math.min(Math.max(0, bounds.left), Math.max(0, window.innerWidth - 1)),
			y: Math.min(Math.max(0, bounds.top), Math.max(0, window.innerHeight - 1)),
			items: targetItems
		};
	}

	function handleGridKeyDown(event: KeyboardEvent) {
		if (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) {
			event.preventDefault();
			handleKeyboardContextMenu();
			return;
		}

		const target = event.target instanceof HTMLElement ? event.target : null;
		if (target?.closest('[data-file-header="true"]') || items.length === 0) return;

		const currentIndex = Math.min(
			items.length - 1,
			Math.max(0, activeItemIndex >= 0 ? activeItemIndex : rowWindow.firstVisibleRow)
		);
		let nextIndex: number;

		switch (event.key) {
			case 'ArrowDown':
				nextIndex = Math.min(items.length - 1, currentIndex + 1);
				break;
			case 'ArrowUp':
				nextIndex = Math.max(0, currentIndex - 1);
				break;
			case 'PageDown':
				nextIndex = Math.min(items.length - 1, currentIndex + rowWindow.visibleRowCount);
				break;
			case 'PageUp':
				nextIndex = Math.max(0, currentIndex - rowWindow.visibleRowCount);
				break;
			case 'Home':
				nextIndex = 0;
				break;
			case 'End':
				nextIndex = items.length - 1;
				break;
			default: {
				const item = items[currentIndex];
				if (!item) return;
				if (event.key === ' ') {
					event.preventDefault();
					handleCheck(item, !selectedPaths.has(item.path));
					return;
				}
				if (event.key === 'Enter') {
					event.preventDefault();
					onItemClick?.(item);
					return;
				}

				let action: 'delete' | 'rename' | 'copy' | 'cut' | 'paste' | null = null;
				if (event.key === 'Delete') action = 'delete';
				else if (event.key === 'F2') action = 'rename';
				else if ((event.key === 'c' || event.key === 'C') && (event.ctrlKey || event.metaKey)) {
					action = 'copy';
				} else if ((event.key === 'x' || event.key === 'X') && (event.ctrlKey || event.metaKey)) {
					action = 'cut';
				} else if ((event.key === 'v' || event.key === 'V') && (event.ctrlKey || event.metaKey)) {
					action = 'paste';
				}
				if (!action) return;

				const targetItems = getKeyboardTargetItems();
				if (targetItems.length === 0) return;
				event.preventDefault();
				onContextMenuAction?.(action, targetItems);
				return;
			}
		}

		event.preventDefault();
		activeItemPath = items[nextIndex]?.path ?? null;
		rowWindow.scrollToItem(nextIndex);
	}

	function handleContextMenu(item: FileInfo, event: MouseEvent) {
		activeItemPath = item.path;
		focusTable();
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
		// The menu item that had focus is about to be removed.
		focusTable();
	}

	function handleContextMenuSelect(action: string) {
		if (contextMenu && onContextMenuAction) {
			onContextMenuAction(action, contextMenu.items);
		}
		contextMenu = null;
		focusTable();
	}

	const thClass =
		'text-left px-3 py-0 bg-surface-secondary border-b border-border-primary font-medium text-text-secondary whitespace-nowrap select-none sticky top-0 z-[5] cursor-pointer transition-colors duration-100 hover:bg-surface-tertiary hover:text-text-primary focus:outline focus:outline-1 focus:outline-accent focus:-outline-offset-1';
	const thSortedClass = 'text-accent';
	const tdClass =
		'h-[32px] px-3 py-1 align-middle border-b border-border-secondary text-text-primary';
	const compactTdClass =
		'h-[28px] px-3 py-0.5 align-middle border-b border-border-secondary text-text-primary';
	const clippedCellClass = 'block overflow-hidden text-ellipsis whitespace-nowrap';
	const selectionRevealClass =
		'flex items-center justify-center group-hover/icon:invisible group-has-[input:focus-visible]/icon:invisible';
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="file-surface relative h-full w-full overflow-auto bg-surface-primary [overflow-anchor:none] {compactMode
		? 'compact'
		: ''}"
	use:attachRowWindow
	onscroll={handleViewportScroll}
	oncontextmenu={handleBackgroundContextMenu}
>
	{#if showSpinner.value}
		<div class="absolute inset-0 z-10 flex items-center justify-center bg-surface-primary/80">
			<Spinner />
		</div>
	{/if}

	<table
		bind:this={tableElement}
		class="w-full min-w-[720px] table-fixed border-collapse text-[13px] leading-5 outline-none"
		role="grid"
		tabindex="0"
		aria-busy={isLoading}
		aria-rowcount={items.length + (items.length === 0 && !isLoading ? 2 : 1)}
		aria-colcount="4"
		aria-multiselectable="true"
		aria-activedescendant={activeItemIndex >= 0 &&
		activeItemIndex >= rowWindow.itemStart &&
		activeItemIndex < rowWindow.itemEnd &&
		activeItemIndex < items.length
			? `file-list-cell-${activeItemIndex}`
			: undefined}
		onfocusin={handleTableFocus}
		onkeydown={handleGridKeyDown}
	>
		<colgroup>
			<col class="file-name-column" />
			<col class="file-type-column" />
			<col class="file-size-column" />
			<col class="file-modified-column" />
		</colgroup>
		<thead>
			<tr aria-rowindex="1" style="height: {LIST_HEADER_HEIGHT}px">
				{#each columns as column (column.field)}
					<th
						class="{thClass} {column.align === 'right' ? 'text-right' : ''} {sortBy === column.field
							? thSortedClass
							: ''}"
						onclick={() => handleSort(column.field)}
						onkeydown={(e) => e.key === 'Enter' && handleSort(column.field)}
						tabindex="0"
						role="columnheader"
						data-file-header="true"
						aria-sort={sortBy === column.field
							? sortDir === 'asc'
								? 'ascending'
								: 'descending'
							: 'none'}
					>
						<span class="inline-flex items-center gap-1">
							{column.label}
							<span class="inline-flex w-3 justify-center">
								{#if sortBy === column.field}
									{#if sortDir === 'asc'}<ChevronUp size={12} />{:else}<ChevronDown
											size={12}
										/>{/if}
								{/if}
							</span>
						</span>
					</th>
				{/each}
			</tr>
		</thead>
		<tbody>
			{#if items.length === 0 && !isLoading}
				<tr aria-rowindex="2">
					<td colspan="4" class="px-3 py-12">
						<div class="flex flex-col items-center gap-2 text-text-muted">
							<FolderOpen size={32} class="opacity-50" />
							<span class="text-[13px]">{emptyMessage}</span>
						</div>
					</td>
				</tr>
			{:else}
				{#if rowWindow.beforeSize > 0}
					<tr aria-hidden="true" style={`height: ${rowWindow.beforeSize}px`}>
						<td colspan="4" class="border-0 p-0" style={`height: ${rowWindow.beforeSize}px`}></td>
					</tr>
				{/if}
				{#each items.slice(rowWindow.itemStart, rowWindow.itemEnd) as item, visibleOffset (item.path)}
					{@const itemIndex = rowWindow.itemStart + visibleOffset}
					{@const IconComponent = getFileIcon(item.name, item.isDir)}
					{@const displayName = fileDisplayName(item, showFileExtensions)}
					{@const typeDescription = item.isDir ? 'Folder' : getFileTypeDescription(item.name)}
					{@const sizeDescription = item.isDir ? '—' : formatFileSize(item.size)}
					{@const secondaryText = isSelected(item.path)
						? 'text-text-primary'
						: 'text-text-secondary'}
					{@const modifiedDescription = formatFileDate(item.modTime)}
					{@const hasThumbnail = supportsThumbnailFile(item)}
					<tr
						style={`height: ${compactMode ? 28 : 32}px`}
						class="cursor-default transition-colors duration-50 {isSelected(item.path)
							? 'bg-selection hover:bg-selection-hover'
							: activeItemIndex === itemIndex
								? 'bg-surface-secondary'
								: 'hover:bg-surface-secondary'} {activeItemIndex === itemIndex
							? 'shadow-[inset_2px_0_0_var(--color-accent)]'
							: ''} {isCut(item.path) ? 'opacity-50' : ''}"
						onpointerdown={() => (activeItemPath = item.path)}
						onclick={(e) => handleRowClick(item, e)}
						onpointerenter={() => item.isDir && onFolderHover?.(item.path)}
						onpointerleave={() => onFolderHover?.(null)}
						ondblclick={() => handleDoubleClick(item)}
						oncontextmenu={(e) => handleContextMenu(item, e)}
						tabindex="-1"
						aria-rowindex={itemIndex + 2}
						aria-selected={isSelected(item.path)}
						data-file-row="true"
						data-file-path={item.path}
					>
						<td id={`file-list-cell-${itemIndex}`} class={compactMode ? compactTdClass : tdClass}>
							<div class="flex min-w-0 items-center gap-2">
								<!-- Selected items keep their checkbox visible after the pointer leaves. -->
								<span
									class="group/icon relative flex h-5 w-5 shrink-0 items-center justify-center {item.isDir
										? 'text-folder'
										: 'text-text-secondary'}"
								>
									<span
										class="flex items-center justify-center {selectionCheckboxes
											? isSelected(item.path)
												? 'invisible'
												: 'group-hover/icon:invisible pointer-coarse:invisible'
											: ''}"
									>
										<IconComponent size={16} />
									</span>
									{#if selectionCheckboxes}
										<!-- Selected items keep their checkbox after the pointer leaves. -->
										<input
											type="checkbox"
											tabindex="-1"
											checked={isSelected(item.path)}
											aria-label={`Select ${item.name}`}
											class="absolute inset-0.5 m-0 cursor-pointer accent-accent {isSelected(
												item.path
											)
												? ''
												: 'opacity-0 group-hover/icon:opacity-100 pointer-coarse:opacity-100'}"
											onclick={(event) => event.stopPropagation()}
											ondblclick={(event) => event.stopPropagation()}
											onchange={(event) => handleCheck(item, event.currentTarget.checked)}
										/>
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
						<td class="{compactMode ? compactTdClass : tdClass} {secondaryText}">
							<span class={clippedCellClass} title={typeDescription}>{typeDescription}</span>
						</td>
						<td
							class="{compactMode
								? compactTdClass
								: tdClass} text-right {secondaryText} tabular-nums"
						>
							<span class={clippedCellClass} title={sizeDescription}>{sizeDescription}</span>
						</td>
						<td class="{compactMode ? compactTdClass : tdClass} {secondaryText}">
							<span class={clippedCellClass} title={modifiedDescription}>{modifiedDescription}</span
							>
						</td>
					</tr>
				{/each}
				{#if rowWindow.afterSize > 0}
					<tr aria-hidden="true" style={`height: ${rowWindow.afterSize}px`}>
						<td colspan="4" class="border-0 p-0" style={`height: ${rowWindow.afterSize}px`}></td>
					</tr>
				{/if}
			{/if}
		</tbody>
	</table>
</div>

<!-- Context Menu -->
{#if contextMenu}
	{#await import('./FileContextMenu.svelte') then { default: FileContextMenu }}
		<FileContextMenu
			items={contextMenu.items}
			{path}
			{canPaste}
			{favoritePaths}
			{canCreate}
			x={contextMenu.x}
			y={contextMenu.y}
			onSelect={collection.selectMenuAction}
			onClose={collection.closeMenu}
		/>
	{:catch}
		<LoadError label="file context menu" onClose={handleContextMenuClose} />
	{/await}
{/if}
