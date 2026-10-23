<script lang="ts">
	import type { FileInfo } from '#lib/api/files.js';
	import FileThumbnail from '#lib/components/FileThumbnail.svelte';
	import { supportsThumbnailFile } from '#lib/utils/thumbnail.js';
	import { formatFileSize, formatFileDate } from '#lib/utils/format.js';
	import { getFileTypeDescription, getFileIcon } from '#lib/utils/fileTypes.js';
	import { SvelteSet } from 'svelte/reactivity';
	import Spinner from '#lib/components/ui/Spinner.svelte';
	import LoadError from '#lib/components/LoadError.svelte';
	import { FolderOpen } from 'lucide-svelte';
	import { CONFIG } from '#lib/config.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';
	import { useFileCollection, fileDisplayName } from '#lib/browse/fileCollection.svelte.js';
	import type { FileContextAction } from '#lib/utils/fileContextMenu.js';
	import type { FileOpenBehavior } from '#lib/utils/fileOpenBehavior.js';
	import { endFileDrag, type FileDropTarget } from '#lib/browse/fileDrag.js';

	let {
		items = [],
		path = '',
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
		dropTarget,
		onItemClick,
		onFolderHover,
		onSelectionChange,
		onContextMenuAction
	}: {
		items?: FileInfo[];
		path?: string;
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
		/** Folders accept dropped files when set. */
		dropTarget?: FileDropTarget;
		onItemClick?: (item: FileInfo) => void;
		/** Path of the folder under the pointer, or null once it leaves */
		onFolderHover?: (path: string | null) => void;
		onSelectionChange?: (paths: Set<string>) => void;
		onContextMenuAction?: (action: FileContextAction, items: FileInfo[]) => void;
	} = $props();

	const showSpinner = createDelayedFlag(() => isLoading, CONFIG.ui.spinnerDelayMs);
	const collection = useFileCollection(
		() => ({
			items,
			selectedPaths,
			fileOpenBehavior,
			dropTarget,
			onItemClick,
			onSelectionChange,
			onContextMenuAction
		}),
		'grid'
	);

	const gridClass = 'grid grid-cols-[repeat(auto-fill,112px)] justify-start gap-3 p-3';
	const compactGridClass = 'grid grid-cols-[repeat(auto-fill,96px)] justify-start gap-2 p-2';
	const tileClass =
		'group relative flex aspect-[1/1.15] min-w-0 cursor-default flex-col rounded border border-transparent bg-transparent p-2 text-left outline-none transition-colors duration-100 focus:border-border-focus';
	const selectedClass = 'border-accent/70 bg-selection hover:bg-selection-hover';
	const unselectedClass = 'hover:bg-surface-secondary focus:bg-surface-secondary';
	const thumbnailClass =
		'flex shrink-0 items-center justify-center overflow-hidden rounded border border-border-primary bg-surface-primary';
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="relative h-full w-full overflow-auto bg-surface-primary"
	oncontextmenu={(event) => collection.backgroundContextMenu(event, '[data-file-item="true"]')}
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
				{@const displayName = fileDisplayName(item, showFileExtensions)}
				{@const typeDescription = item.isDir ? 'Folder' : getFileTypeDescription(item.name)}
				{@const sizeDescription = item.isDir ? '' : formatFileSize(item.size)}
				{@const modifiedDescription = formatFileDate(item.modTime)}
				{@const showThumbnail = supportsThumbnailFile(item)}
				{@const detailText = item.isDir ? '' : sizeDescription}
				<div
					class="file-grid-item group/tile {tileClass} {selectedPaths.has(item.path)
						? selectedClass
						: unselectedClass} {cutPaths.has(item.path)
						? 'opacity-50'
						: ''} {collection.isDropTarget(item) ? 'border-accent bg-accent/15' : ''}"
					draggable="true"
					ondragstart={(event) => collection.dragStart(item, event)}
					ondragend={endFileDrag}
					ondragover={(event) => collection.dragOver(item, event)}
					ondragleave={(event) => collection.dragLeave(item, event)}
					ondrop={(event) => collection.drop(item, event)}
					role="gridcell"
					tabindex="0"
					aria-selected={selectedPaths.has(item.path)}
					title={`${item.name}\n${typeDescription}${sizeDescription ? ` - ${sizeDescription}` : ''}\nModified ${modifiedDescription}`}
					onclick={(event) => collection.click(item, event)}
					onpointerenter={() => item.isDir && onFolderHover?.(item.path)}
					onpointerleave={() => onFolderHover?.(null)}
					ondblclick={(event) => collection.doubleClick(item, event)}
					onkeydown={(event) => collection.keydown(item, event)}
					oncontextmenu={(event) => collection.itemContextMenu(item, event)}
					data-file-item="true"
					data-file-path={item.path}
				>
					<input
						type="checkbox"
						checked={selectedPaths.has(item.path)}
						aria-label={`Select ${item.name}`}
						class="absolute top-2 left-2 z-[1] h-4 w-4 cursor-pointer accent-accent {selectedPaths.has(
							item.path
						)
							? ''
							: 'opacity-0 group-hover/tile:opacity-100 focus-visible:opacity-100 pointer-coarse:opacity-100'}"
						onclick={(event) => event.stopPropagation()}
						ondblclick={(event) => event.stopPropagation()}
						onchange={(event) => collection.check(item, event.currentTarget.checked)}
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
						<div class="file-grid-name h-8 text-center text-[13px] leading-4 text-text-primary">
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

{#if collection.contextMenu}
	{@const contextMenu = collection.contextMenu}
	{#await import('#lib/components/FileContextMenu.svelte') then { default: FileContextMenu }}
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
		<LoadError label="file menu" onClose={collection.closeMenu} />
	{/await}
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
