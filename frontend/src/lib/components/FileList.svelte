<script lang="ts">
	/**
	 * FileList component with sortable columns - FilePilot style
	 */

	import {
		startFileDrag,
		endFileDrag,
		isDragged,
		type FileDropTarget
	} from '#lib/browse/fileDrag.js';
	import type { FileInfo } from '#lib/api/files.js';
	import { DEFAULT_SORT_BY, DEFAULT_SORT_DIR } from '#lib/browse/sortFiles.js';
	import type { SortField, SortDir } from '#lib/types/files.js';
	import { formatFileSize, formatFileDate } from '#lib/utils/format.js';
	import { getFileTypeDescription, getFileIcon, getFileTint } from '#lib/utils/fileTypes.js';
	import { SvelteSet } from 'svelte/reactivity';
	import { untrack } from 'svelte';
	import { Spinner } from '#lib/components/ui/index.js';
	import LoadError from '#lib/components/LoadError.svelte';
	import { FolderOpen } from 'lucide-svelte';
	import { CONFIG } from '#lib/config.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';
	import { createFixedRowWindow } from '#lib/utils/fixedRowWindow.svelte.js';
	import { ViewCursor } from '#lib/browse/viewCursor.svelte.js';
	import { RubberBand } from '#lib/browse/rubberBand.svelte.js';
	import { isMiddleButton, openInNewTab } from '#lib/browse/newTab.js';
	import { dragScrollStep } from '#lib/browse/springLoad.js';
	import { DEFAULT_ZOOM, listMetrics } from '#lib/browse/zoom.js';
	import type { FolderAppearance } from '#lib/browse/folderLooks.js';
	import FolderGlyph from './FolderGlyph.svelte';
	import type { RenameEditor } from '#lib/browse/renameEditor.js';
	import { hoverPoint, type HoverTarget } from '#lib/browse/hoverPoint.js';
	import { ColumnTable } from '#lib/browse/columnTable.svelte.js';
	import type { Column } from '#lib/browse/columns.js';
	import ColumnHeader from './ColumnHeader.svelte';

	const PLAIN_FOLDER: FolderAppearance = { color: null, emblem: null, emblemColor: null };

	let {
		items = [],
		viewKey,
		sortBy = DEFAULT_SORT_BY,
		sortDir = DEFAULT_SORT_DIR,
		emptyMessage = 'This folder is empty',
		selectedPaths = new SvelteSet<string>(),
		isLoading = false,
		zoom = DEFAULT_ZOOM,
		cutPaths = new SvelteSet<string>(),
		favoritePaths = new SvelteSet<string>(),
		canPaste = false,
		canCreate = false,
		readOnly = false,
		showFileExtensions = true,
		previewOnSingleClick = false,
		selectionCheckboxes = true,
		fileIconTints = false,
		dropTarget,
		folderAppearance = () => PLAIN_FOLDER,
		hover,
		renameEditor = null,
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
		/** The list's zoom step. */
		zoom?: number;
		cutPaths?: Set<string>;
		favoritePaths?: Set<string>;
		canPaste?: boolean;
		canCreate?: boolean;
		readOnly?: boolean;
		showFileExtensions?: boolean;
		previewOnSingleClick?: boolean;
		selectionCheckboxes?: boolean;
		/** Files take their kind's colour; see `getFileTint`. */
		fileIconTints?: boolean;
		dropTarget?: FileDropTarget;
		/** How a folder is drawn: its colour and emblem. */
		folderAppearance?: (path: string) => FolderAppearance;
		/** The hover card; without it, a row's native tooltip names the item. */
		hover?: HoverTarget;
		/** The item being renamed in place, if any. */
		renameEditor?: RenameEditor | null;
		onItemClick?: (item: FileInfo) => void;
		/** Path of the folder under the pointer, or null once it leaves */
		onFolderHover?: (path: string | null) => void;
		onSortChange?: (field: SortField, dir: SortDir) => void;
		onSelectionChange?: (paths: Set<string>) => void;
		onContextMenuAction?: (action: string, items: FileInfo[]) => void;
	} = $props();

	const showSpinner = createDelayedFlag(() => isLoading, CONFIG.ui.spinnerDelayMs);
	const LIST_HEADER_HEIGHT = 32;
	const columns = new ColumnTable();
	const observeColumns = columns.observe;
	const SORT_FIELDS: Partial<Record<Column, SortField>> = {
		name: 'name',
		size: 'size',
		type: 'type',
		modified: 'modTime'
	};
	const sortedColumn = $derived(
		(Object.keys(SORT_FIELDS) as Column[]).find((column) => SORT_FIELDS[column] === sortBy) ?? null
	);
	let tableElement = $state<HTMLTableElement | null>(null);
	const metrics = $derived(listMetrics(zoom));
	const cursor = new ViewCursor(() => ({ items, selectedPaths, onSelectionChange }));
	const activeItemIndex = $derived(cursor.activeIndex);
	const band = new RubberBand(() => ({
		itemsTop: LIST_HEADER_HEIGHT,
		layout: () => ({ kind: 'list', rowHeight: metrics.rowHeight }),
		items: () => items,
		selected: () => selectedPaths,
		select: (paths) => onSelectionChange?.(paths),
		focus: focusTable
	}));
	const attachBand = band.attach;
	const rowWindow = createFixedRowWindow({
		items: () => items,
		itemKey: (item) => item.path,
		viewKey: () => viewKey,
		isReady: () => !isLoading,
		rowHeight: () => metrics.rowHeight,
		itemOffset: () => LIST_HEADER_HEIGHT,
		viewportOffset: () => LIST_HEADER_HEIGHT
	});
	const attachRowWindow = rowWindow.attach;
	$effect(() => {
		const path = cursor.activePath;
		if (path === null || items.some((item) => item.path === path)) return;
		untrack(() => {
			cursor.activePath = null;
		});
	});

	// Context menu state
	let contextMenu = $state<{ x: number; y: number; items: FileInfo[] } | null>(null);

	function handleSort(column: Column) {
		const field = SORT_FIELDS[column];
		if (!field) return;
		if (sortBy === field) {
			const newDir = sortDir === 'asc' ? 'desc' : 'asc';
			onSortChange?.(field, newDir);
		} else {
			onSortChange?.(field, 'asc');
		}
	}

	function handleRowClick(item: FileInfo, event: MouseEvent) {
		focusTable();
		if (cursor.click(item, event) !== 'replaced') return;
		// A plain click opens a folder, or a file when Settings preview on one click.
		if (item.isDir || previewOnSingleClick) onItemClick?.(item);
	}

	function handleCheck(item: FileInfo) {
		cursor.toggle(item);
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
		if (items.length === 0 || cursor.activePath !== null) return;
		const firstVisible = Math.min(items.length - 1, rowWindow.firstVisibleRow);
		cursor.activePath = items[firstVisible]?.path ?? null;
	}

	function handleViewportScroll() {
		rowWindow.onScroll();
	}

	/** The Menu key: the selection's menu by its item, or the folder's when nothing is picked. */
	function handleKeyboardContextMenu() {
		if (!tableElement) return;
		const targetItems = cursor.targets();
		const activeIsVisible =
			activeItemIndex >= rowWindow.firstVisibleRow && activeItemIndex < rowWindow.lastVisibleRow;
		const activeCell =
			activeIsVisible && targetItems.length > 0
				? tableElement.querySelector<HTMLElement>(`#file-list-cell-${activeItemIndex}`)
				: null;
		const bounds =
			activeCell?.getBoundingClientRect() ??
			tableElement.parentElement?.getBoundingClientRect() ??
			tableElement.getBoundingClientRect();
		contextMenu = {
			x: Math.min(Math.max(0, bounds.left + 40), Math.max(0, window.innerWidth - 1)),
			y: Math.min(Math.max(0, bounds.top + 8), Math.max(0, window.innerHeight - 1)),
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

		const moved = cursor.key(
			event,
			{ grid: false, columns: 1, page: Math.max(1, rowWindow.visibleRowCount) },
			performance.now()
		);
		if (moved.kind !== 'ignored') {
			event.preventDefault();
			if (moved.kind === 'moved') rowWindow.scrollToItem(moved.index);
			return;
		}

		const item = items[activeItemIndex];
		if (event.key === 'Enter' && !event.altKey && item) {
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

		const targetItems = cursor.targets();
		if (targetItems.length === 0) return;
		event.preventDefault();
		onContextMenuAction?.(action, targetItems);
	}

	function handleContextMenu(item: FileInfo, event: MouseEvent) {
		focusTable();
		event.preventDefault();
		// A right-click on an item that is not picked picks only it.
		cursor.point(item);

		// Get all selected items for context menu
		const selectedItems = items.filter((i) => selectedPaths.has(i.path) || i.path === item.path);

		contextMenu = {
			x: event.clientX,
			y: event.clientY,
			items: selectedItems.length > 0 ? selectedItems : [item]
		};
	}

	function handleBackgroundContextMenu(event: MouseEvent) {
		const target = event.target instanceof HTMLElement ? event.target : null;
		if (target?.closest('[data-file-row="true"], [data-file-header="true"]')) return;

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

	const tdClass = 'px-3 py-0 align-middle border-b border-border-secondary text-text-primary';
	const clippedCellClass = 'block overflow-hidden text-ellipsis whitespace-nowrap';
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="file-surface relative h-full w-full overflow-auto bg-surface-primary [overflow-anchor:none]"
	use:attachRowWindow
	use:observeColumns
	use:attachBand={viewKey}
	onscroll={handleViewportScroll}
	ondragovercapture={(event) => {
		// Near the top or bottom, a drag scrolls the view; the drop is the pane's to take.
		const view = event.currentTarget;
		const bounds = view.getBoundingClientRect();
		view.scrollTop += dragScrollStep(event.clientY, bounds.top + LIST_HEADER_HEIGHT, bounds.bottom);
	}}
	oncontextmenu={handleBackgroundContextMenu}
>
	{#if band.rect}
		<div
			class="pointer-events-none absolute z-[6] rounded-[2px] border border-accent bg-accent/15"
			style:left="{band.rect.left}px"
			style:top="{band.rect.top}px"
			style:width="{band.rect.width}px"
			style:height="{band.rect.height}px"
			aria-hidden="true"
		></div>
	{/if}

	{#if showSpinner.value}
		<div class="absolute inset-0 z-10 flex items-center justify-center bg-surface-primary/80">
			<Spinner />
		</div>
	{/if}

	<table
		bind:this={tableElement}
		class="w-full table-fixed border-collapse text-[13px] leading-5 outline-none"
		style:min-width="{columns.minWidth}px"
		role="grid"
		tabindex="0"
		aria-busy={isLoading}
		aria-rowcount={items.length + (items.length === 0 && !isLoading ? 2 : 1)}
		aria-colcount={columns.specs.length}
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
		<ColumnHeader
			{columns}
			table={tableElement}
			height={LIST_HEADER_HEIGHT}
			sortedBy={sortedColumn}
			{sortDir}
			onSort={handleSort}
		/>
		<tbody>
			{#if items.length === 0 && !isLoading}
				<tr aria-rowindex="2">
					<td colspan={columns.specs.length} class="px-3 py-12">
						<div class="flex flex-col items-center gap-2 text-text-muted">
							<FolderOpen size={32} class="opacity-50" />
							<span class="text-[13px]">{emptyMessage}</span>
						</div>
					</td>
				</tr>
			{:else}
				{#if rowWindow.beforeSize > 0}
					<tr aria-hidden="true" style={`height: ${rowWindow.beforeSize}px`}>
						<td
							colspan={columns.specs.length}
							class="border-0 p-0"
							style={`height: ${rowWindow.beforeSize}px`}
						></td>
					</tr>
				{/if}
				{#each items.slice(rowWindow.itemStart, rowWindow.itemEnd) as item, visibleOffset (item.path)}
					{@const itemIndex = rowWindow.itemStart + visibleOffset}
					{@const IconComponent = getFileIcon(item.name, item.isDir)}
					{@const displayName = getDisplayName(item)}
					{@const typeDescription = item.isDir ? 'Folder' : getFileTypeDescription(item.name)}
					{@const sizeDescription = item.isDir ? '—' : formatFileSize(item.size)}
					{@const secondaryText = isSelected(item.path)
						? 'text-text-primary'
						: 'text-text-secondary'}
					{@const modifiedDescription = formatFileDate(item.modTime)}
					<tr
						style={`height: ${metrics.rowHeight}px`}
						class="cursor-default transition-colors duration-50 {isSelected(item.path)
							? 'bg-selection hover:bg-selection-hover'
							: activeItemIndex === itemIndex
								? 'bg-surface-secondary'
								: 'hover:bg-surface-secondary'} {activeItemIndex === itemIndex
							? 'shadow-[inset_2px_0_0_var(--color-accent)]'
							: ''} {isCut(item.path) ? 'opacity-50' : ''}"
						onpointerdown={() => (cursor.activePath = item.path)}
						onclick={(e) => handleRowClick(item, e)}
						onpointerenter={(event) => {
							if (item.isDir) onFolderHover?.(item.path);
							hover?.rest(item, hoverPoint(event));
						}}
						onpointermove={(event) => hover?.rest(item, hoverPoint(event))}
						onpointerleave={() => {
							onFolderHover?.(null);
							hover?.hide();
						}}
						ondblclick={() => handleDoubleClick(item)}
						class:ring-1={dropTarget?.overPath === item.path}
						class:ring-accent={dropTarget?.overPath === item.path}
						draggable={!!dropTarget}
						ondragstart={(event) => {
							if (!event.dataTransfer) return;
							const selected = selectedPaths.has(item.path)
								? items.filter((entry) => selectedPaths.has(entry.path))
								: [item];
							startFileDrag(event.dataTransfer, selected);
						}}
						ondragend={endFileDrag}
						ondragover={(event) => {
							if (isDragged(item.path)) {
								dropTarget?.refuse(event);
								return;
							}
							if (item.isDir && dropTarget?.over(event, item.path)) event.stopPropagation();
						}}
						ondragleave={(event) => item.isDir && dropTarget?.leave(event)}
						ondrop={(event) => {
							if (isDragged(item.path)) dropTarget?.refuse(event);
							else if (item.isDir) dropTarget?.drop(event, item.path);
						}}
						oncontextmenu={(e) => handleContextMenu(item, e)}
						tabindex="-1"
						aria-rowindex={itemIndex + 2}
						aria-selected={isSelected(item.path)}
						data-file-row="true"
						onmousedown={(event) => {
							if (isMiddleButton(event) && item.isDir) event.preventDefault();
						}}
						onauxclick={(event) => {
							if (isMiddleButton(event) && item.isDir) openInNewTab(item.path);
						}}
						data-file-path={item.path}
					>
						{#each columns.specs as spec (spec.column)}
							{#if spec.column === 'name'}
								<td id={`file-list-cell-${itemIndex}`} class={tdClass}>
									<div class="flex min-w-0 items-center gap-2">
										<span
											style:width="{metrics.icon + 4}px"
											style:height="{metrics.icon + 4}px"
											style:color={fileIconTints ? getFileTint(item.name, item.isDir) : null}
											class="group/icon relative flex shrink-0 items-center justify-center {item.isDir
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
												{#if item.isDir}
													<FolderGlyph size={metrics.icon} {...folderAppearance(item.path)} />
												{:else}
													<IconComponent size={metrics.icon} />
												{/if}
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
													onchange={() => handleCheck(item)}
												/>
											{/if}
										</span>
										{#if renameEditor?.path === item.path}
											{#await import('./InlineNameEditor.svelte') then { default: InlineNameEditor }}
												<InlineNameEditor
													value={renameEditor.name}
													isDir={renameEditor.isDir}
													problem={renameEditor.problem}
													oninput={renameEditor.oninput}
													oncommit={() => {
														renameEditor.oncommit();
														focusTable();
													}}
													oncancel={() => {
														renameEditor.oncancel();
														focusTable();
													}}
												/>
											{/await}
										{:else}
											<span
												class="{clippedCellClass} min-w-0 flex-1"
												title={hover ? undefined : item.name}
											>
												{displayName}
											</span>
										{/if}
									</div>
								</td>
							{:else if spec.column === 'size'}
								<td class="{tdClass} text-right {secondaryText} tabular-nums">
									<span class={clippedCellClass} title={sizeDescription}>{sizeDescription}</span>
								</td>
							{:else if spec.column === 'type'}
								<td class="{tdClass} {secondaryText}">
									<span class={clippedCellClass} title={typeDescription}>{typeDescription}</span>
								</td>
							{:else}
								<td class="{tdClass} {secondaryText}">
									<span class={clippedCellClass} title={modifiedDescription}
										>{modifiedDescription}</span
									>
								</td>
							{/if}
						{/each}
					</tr>
				{/each}
				{#if rowWindow.afterSize > 0}
					<tr aria-hidden="true" style={`height: ${rowWindow.afterSize}px`}>
						<td
							colspan={columns.specs.length}
							class="border-0 p-0"
							style={`height: ${rowWindow.afterSize}px`}
						></td>
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
			{canPaste}
			{favoritePaths}
			{canCreate}
			{readOnly}
			x={contextMenu.x}
			y={contextMenu.y}
			onSelect={handleContextMenuSelect}
			onClose={handleContextMenuClose}
		/>
	{:catch}
		<LoadError label="file context menu" onClose={handleContextMenuClose} />
	{/await}
{/if}
