<script lang="ts">
	/**
	 * FileGrid component - icon/thumbnail grid for browsing folders
	 */

	import {
		startFileDrag,
		endFileDrag,
		isDragged,
		type FileDropTarget
	} from '#lib/browse/fileDrag.js';
	import type { FileInfo } from '#lib/api/files.js';
	import { getThumbnailUrl } from '#lib/api/files.js';
	import { formatFileSize, formatFileDate } from '#lib/utils/format.js';
	import {
		getFileTypeDescription,
		getFileIcon,
		getFileTint,
		getPreviewType
	} from '#lib/utils/fileTypes.js';
	import { SvelteSet } from 'svelte/reactivity';
	import { onMount, untrack } from 'svelte';
	import { canvasMeasure } from '#lib/utils/textMeasure.js';
	import type { Measure } from '#lib/browse/tileLabel.js';
	import TileLabel from './TileLabel.svelte';
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
	import { DEFAULT_ZOOM, gridMetrics } from '#lib/browse/zoom.js';
	import type { FolderAppearance } from '#lib/browse/folderLooks.js';
	import FolderGlyph from './FolderGlyph.svelte';
	import type { RenameEditor } from '#lib/browse/renameEditor.js';
	import FolderPeekCards from './FolderPeekCards.svelte';
	import { folderPeeks } from '#lib/browse/folderPeek.svelte.js';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { hoverPoint, type HoverTarget } from '#lib/browse/hoverPoint.js';

	const PLAIN_FOLDER: FolderAppearance = { color: null, emblem: null, emblemColor: null };

	let {
		items = [],
		viewKey,
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
		folderPreviews = false,
		onItemClick,
		onFolderHover,
		onSelectionChange,
		onContextMenuAction
	}: {
		items?: FileInfo[];
		viewKey: string;
		emptyMessage?: string;
		selectedPaths?: Set<string>;
		isLoading?: boolean;
		/** The grid's zoom step. */
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
		/** Folders draw what they hold inside their tiles. */
		folderPreviews?: boolean;
		onItemClick?: (item: FileInfo) => void;
		/** Path of the folder under the pointer, or null once it leaves */
		onFolderHover?: (path: string | null) => void;
		onSelectionChange?: (paths: Set<string>) => void;
		onContextMenuAction?: (action: string, items: FileInfo[]) => void;
	} = $props();

	const showSpinner = createDelayedFlag(() => isLoading, CONFIG.ui.spinnerDelayMs);
	let gridElement = $state<HTMLDivElement | null>(null);
	const metrics = $derived(gridMetrics(zoom));
	const showHidden = $derived(settingsStore.current.showHiddenFiles);
	const peeks = $derived(
		folderPreviews
			? {
					get: (folder: FileInfo) => folderPeeks.get(folder, showHidden),
					request: (folder: FileInfo) => folderPeeks.request(folder, showHidden)
				}
			: undefined
	);
	const cursor = new ViewCursor(() => ({ items, selectedPaths, onSelectionChange }));
	const activeItemIndex = $derived(cursor.activeIndex);
	const band = new RubberBand(() => ({
		itemsTop: 0,
		layout: () => ({ kind: 'grid', metrics, columns: rowWindow.columnCount }),
		items: () => items,
		selected: () => selectedPaths,
		select: (paths) => onSelectionChange?.(paths),
		focus: () => gridElement?.focus({ preventScroll: true })
	}));
	const attachBand = band.attach;
	const rowWindow = createFixedRowWindow({
		items: () => items,
		itemKey: (item) => item.path,
		viewKey: () => viewKey,
		isReady: () => !isLoading,
		rowHeight: () => metrics.rowHeight,
		itemOffset: () => metrics.padding,
		contentEndPadding: () => metrics.padding,
		columnsForWidth: (width) =>
			Math.max(
				1,
				Math.floor((width - metrics.padding * 2 + metrics.gap) / (metrics.tileWidth + metrics.gap))
			)
	});
	const attachRowWindow = rowWindow.attach;
	// Labels are laid out in the page's own font, once it has loaded.
	let measureLabel = $state.raw<Measure | null>(null);
	onMount(() => {
		let live = true;
		void document.fonts.ready.then(() => {
			if (live && gridElement)
				measureLabel = canvasMeasure(`13px ${getComputedStyle(gridElement).fontFamily}`);
		});
		return () => {
			live = false;
		};
	});
	$effect(() => {
		const path = cursor.activePath;
		if (path === null || items.some((item) => item.path === path)) return;
		untrack(() => {
			cursor.activePath = null;
		});
	});

	let contextMenu = $state<{ x: number; y: number; items: FileInfo[] } | null>(null);
	let thumbnailErrors = new SvelteSet<string>();

	function handleItemClick(item: FileInfo, event: MouseEvent) {
		gridElement?.focus({ preventScroll: true });
		if (cursor.click(item, event) !== 'replaced') return;
		// A plain click opens a folder, or a file when Settings preview on one click.
		if (item.isDir || previewOnSingleClick) onItemClick?.(item);
	}

	function handleCheck(item: FileInfo) {
		cursor.toggle(item);
		// Keyboard navigation stays on the grid instead of the checkbox.
		gridElement?.focus({ preventScroll: true });
	}

	function handleOpen(item: FileInfo) {
		if (previewOnSingleClick && !item.isDir) return;
		onItemClick?.(item);
	}

	function handleGridFocus() {
		if (items.length === 0 || cursor.activePath !== null) return;
		const firstVisible = Math.min(
			items.length - 1,
			rowWindow.firstVisibleRow * rowWindow.columnCount
		);
		cursor.activePath = items[firstVisible]?.path ?? null;
	}

	function handleViewportScroll() {
		rowWindow.onScroll();
	}

	/** The Menu key: the selection's menu by its item, or the folder's when nothing is picked. */
	function handleKeyboardContextMenu() {
		if (!gridElement) return;
		const targetItems = cursor.targets();
		const activeRow = Math.floor(activeItemIndex / rowWindow.columnCount);
		const activeIsVisible =
			targetItems.length > 0 &&
			activeItemIndex >= 0 &&
			activeRow >= rowWindow.firstVisibleRow &&
			activeRow < rowWindow.lastVisibleRow;
		const activeCell = activeIsVisible
			? gridElement.querySelector<HTMLElement>(`#file-grid-cell-${activeItemIndex}`)
			: null;
		const bounds =
			activeCell?.getBoundingClientRect() ??
			gridElement.parentElement?.getBoundingClientRect() ??
			gridElement.getBoundingClientRect();
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
		if (items.length === 0) return;

		const columns = rowWindow.columnCount;
		const moved = cursor.key(
			event,
			{ grid: true, columns, page: columns * Math.max(1, rowWindow.visibleRowCount) },
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
		gridElement?.focus({ preventScroll: true });
		event.preventDefault();
		// A right-click on an item that is not picked picks only it.
		cursor.point(item);

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
		// The menu item that had focus is about to be removed.
		gridElement?.focus({ preventScroll: true });
	}

	function handleContextMenuSelect(action: string) {
		if (contextMenu && onContextMenuAction) {
			onContextMenuAction(action, contextMenu.items);
		}
		contextMenu = null;
		gridElement?.focus({ preventScroll: true });
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

	const tileClass =
		'group/tile relative flex min-w-0 cursor-default flex-col rounded border p-2 text-left outline-none transition-colors duration-100';
	// One state sets the colours, so a tile never carries two competing backgrounds.
	const selectedClass = 'border-accent/70 bg-selection hover:bg-selection-hover';
	const activeClass = 'border-transparent bg-surface-secondary';
	const idleClass = 'border-transparent hover:bg-surface-secondary';
	const thumbnailClass =
		'flex shrink-0 items-center justify-center overflow-hidden rounded border border-border-primary bg-surface-primary';
	const iconSlotClass = 'flex shrink-0 items-center justify-center';
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="file-surface relative h-full w-full overflow-auto bg-surface-primary [overflow-anchor:none]"
	use:attachRowWindow
	use:attachBand={viewKey}
	onscroll={handleViewportScroll}
	ondragovercapture={(event) => {
		// Near the top or bottom, a drag scrolls the view; the drop is the pane's to take.
		const view = event.currentTarget;
		const bounds = view.getBoundingClientRect();
		view.scrollTop += dragScrollStep(event.clientY, bounds.top, bounds.bottom);
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

	{#if items.length === 0 && !isLoading}
		<div class="flex h-full flex-col items-center justify-center gap-2 text-text-muted">
			<FolderOpen size={32} class="opacity-50" />
			<span class="text-[13px]">{emptyMessage}</span>
		</div>
	{:else}
		<div
			bind:this={gridElement}
			class="flex flex-col outline-none"
			style:padding="{metrics.padding}px"
			role="grid"
			tabindex="0"
			aria-busy={isLoading}
			aria-multiselectable="true"
			aria-rowcount={rowWindow.rowCount}
			aria-colcount={rowWindow.columnCount}
			aria-activedescendant={activeItemIndex >= 0 &&
			activeItemIndex >= rowWindow.itemStart &&
			activeItemIndex < rowWindow.itemEnd &&
			activeItemIndex < items.length
				? `file-grid-cell-${activeItemIndex}`
				: undefined}
			onfocusin={handleGridFocus}
			onkeydown={handleGridKeyDown}
		>
			{#if rowWindow.beforeSize > 0}
				<div
					aria-hidden="true"
					role="presentation"
					class="shrink-0"
					style={`height: ${rowWindow.beforeSize}px`}
				></div>
			{/if}
			{#each rowWindow.visibleRows as rowIndex (rowIndex)}
				{@const firstItemIndex = rowIndex * rowWindow.columnCount}
				{@const rowItems = items.slice(
					firstItemIndex,
					Math.min(items.length, firstItemIndex + rowWindow.columnCount)
				)}
				<div
					class="grid grid-flow-row"
					style:height="{metrics.rowHeight}px"
					style:column-gap="{metrics.gap}px"
					style:grid-template-columns="repeat({rowWindow.columnCount}, {metrics.tileWidth}px)"
					role="row"
					aria-rowindex={rowIndex + 1}
				>
					{#each rowItems as item, columnOffset (item.path)}
						{@const itemIndex = firstItemIndex + columnOffset}
						{@const IconComponent = getFileIcon(item.name, item.isDir)}
						{@const displayName = getDisplayName(item)}
						{@const typeDescription = item.isDir ? 'Folder' : getFileTypeDescription(item.name)}
						{@const sizeDescription = item.isDir ? '' : formatFileSize(item.size)}
						{@const modifiedDescription = formatFileDate(item.modTime)}
						{@const showThumbnail =
							!item.isDir &&
							getPreviewType(item.name) === 'image' &&
							!thumbnailErrors.has(item.path)}
						{@const detailText = sizeDescription}
						<!-- A div, because the selection checkbox cannot sit inside a button. The grid owns the keys. -->
						<!-- svelte-ignore a11y_click_events_have_key_events -->
						<div
							id={`file-grid-cell-${itemIndex}`}
							class="{tileClass} {isSelected(item.path)
								? selectedClass
								: activeItemIndex === itemIndex
									? activeClass
									: idleClass} {isCut(item.path) ? 'opacity-50' : ''}"
							style={`height: ${metrics.rowHeight - metrics.padding}px`}
							role="gridcell"
							tabindex="-1"
							aria-colindex={columnOffset + 1}
							aria-selected={isSelected(item.path)}
							title={hover
								? undefined
								: `${item.name}\n${typeDescription}${sizeDescription ? ` - ${sizeDescription}` : ''}\nModified ${modifiedDescription}`}
							onpointerdown={() => (cursor.activePath = item.path)}
							onclick={(e) => handleItemClick(item, e)}
							onpointerenter={(event) => {
								if (item.isDir) onFolderHover?.(item.path);
								hover?.rest(item, hoverPoint(event));
							}}
							onpointermove={(event) => hover?.rest(item, hoverPoint(event))}
							onpointerleave={() => {
								onFolderHover?.(null);
								hover?.hide();
							}}
							ondblclick={() => handleOpen(item)}
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
							data-file-item="true"
							onmousedown={(event) => {
								if (isMiddleButton(event) && item.isDir) event.preventDefault();
							}}
							onauxclick={(event) => {
								if (isMiddleButton(event) && item.isDir) openInNewTab(item.path);
							}}
							data-file-path={item.path}
						>
							{#if selectionCheckboxes}
								<input
									type="checkbox"
									tabindex="-1"
									checked={isSelected(item.path)}
									aria-label={`Select ${item.name}`}
									class="absolute top-2 left-2 z-[1] m-0 h-4 w-4 cursor-pointer accent-accent {isSelected(
										item.path
									)
										? ''
										: 'opacity-0 group-hover/tile:opacity-100 pointer-coarse:opacity-100'}"
									onclick={(event) => event.stopPropagation()}
									ondblclick={(event) => event.stopPropagation()}
									onchange={() => handleCheck(item)}
								/>
							{/if}
							<!-- A thumbnail sits in a frame; an icon stands on its own, as in Thunar and the desktop app. -->
							<div
								class={showThumbnail ? thumbnailClass : iconSlotClass}
								style:height="{metrics.previewHeight}px"
							>
								{#if showThumbnail}
									<img
										src={getThumbnailUrl(
											item.path,
											metrics.thumbnail.width,
											metrics.thumbnail.height
										)}
										alt=""
										loading="lazy"
										class="h-full w-full object-cover"
										onerror={() => handleThumbnailError(item.path)}
									/>
								{:else}
									<span
										style:color={fileIconTints ? getFileTint(item.name, item.isDir) : null}
										class="flex h-full w-full items-center justify-center {item.isDir
											? 'text-folder'
											: 'text-text-secondary'}"
									>
										{#if item.isDir}
											{@const look = folderAppearance(item.path)}
											{@const peek = look.emblem === null ? peeks?.get(item) : undefined}
											<!-- A folder with an emblem keeps it instead of its contents. -->
											<span
												class="relative inline-flex"
												{@attach () => {
													if (look.emblem === null) untrack(() => peeks?.request(item));
												}}
											>
												<FolderGlyph size={metrics.icon} strokeWidth={1.7} {...look} />
												{#if peek?.length}
													<FolderPeekCards items={peek} size={metrics.icon} />
												{/if}
											</span>
										{:else}
											<IconComponent size={metrics.icon} strokeWidth={1.7} />
										{/if}
									</span>
								{/if}
							</div>

							<div class="mt-1.5 min-w-0 overflow-hidden">
								<div class="text-center text-[13px] leading-4 text-text-primary">
									{#if renameEditor?.path === item.path}
										{#await import('./InlineNameEditor.svelte') then { default: InlineNameEditor }}
											<InlineNameEditor
												value={renameEditor.name}
												isDir={renameEditor.isDir}
												problem={renameEditor.problem}
												align="center"
												oninput={renameEditor.oninput}
												oncommit={() => {
													renameEditor.oncommit();
													gridElement?.focus({ preventScroll: true });
												}}
												oncancel={() => {
													renameEditor.oncancel();
													gridElement?.focus({ preventScroll: true });
												}}
											/>
										{/await}
									{:else}
										<TileLabel
											name={displayName}
											width={metrics.tileWidth - 18}
											measure={measureLabel}
										/>
									{/if}
								</div>
								<div
									class="mt-0.5 truncate text-center text-[11px] leading-3.5 {isSelected(item.path)
										? 'text-text-primary'
										: 'text-text-muted'}"
									title={typeDescription}
								>
									{detailText}
								</div>
							</div>
						</div>
					{/each}
				</div>
			{/each}
			{#if rowWindow.afterSize > 0}
				<div
					aria-hidden="true"
					role="presentation"
					class="shrink-0"
					style={`height: ${rowWindow.afterSize}px`}
				></div>
			{/if}
		</div>
	{/if}
</div>

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
