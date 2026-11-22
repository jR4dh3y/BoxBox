<script lang="ts">
	/**
	 * The Details panel at the right, as Dolphin's Information panel and the desktop app's (F11):
	 * the selection, or the folder when nothing is selected. From the top: the preview, the name,
	 * then the details as label and value columns. Values can be selected and copied.
	 */
	import { getThumbnailUrl, type FileInfo } from '#lib/api/files.js';
	import { detailsContent, fullFileDate, parentFolder } from '#lib/browse/details.js';
	import type { FolderAppearance } from '#lib/browse/folderLooks.js';
	import { formatFileSize } from '#lib/utils/format.js';
	import {
		getFileIcon,
		getFileTint,
		getFileTypeDescription,
		getPreviewType
	} from '#lib/utils/fileTypes.js';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import FolderGlyph from './FolderGlyph.svelte';

	let {
		selected,
		folderCount,
		folderName,
		appearance,
		locationLabel
	}: {
		selected: readonly FileInfo[];
		/** How many items the open folder lists. */
		folderCount: number;
		/** The open folder's name, or the drives' title at the root. */
		folderName: string;
		appearance: (path: string) => FolderAppearance;
		/** A path as the path bar names it. */
		locationLabel: (path: string) => string;
	} = $props();

	const content = $derived(detailsContent(selected, folderCount));
	let failedThumbnail = $state<string | null>(null);
	const counted = (count: number, noun: string) => `${count} ${noun}${count === 1 ? '' : 's'}`;

	function rows(item: FileInfo): [string, string][] {
		const list: [string, string][] = [
			['Type', item.isDir ? 'Folder' : getFileTypeDescription(item.name)]
		];
		if (!item.isDir) list.push(['Size', formatFileSize(item.size)]);
		list.push(['Location', locationLabel(parentFolder(item.path))]);
		list.push(['Modified', fullFileDate(item.modTime)]);
		return list;
	}
</script>

{#snippet details(list: [string, string][])}
	<div class="mt-[18px] flex flex-col">
		<h3 class="m-0 mb-1 text-[13px] font-medium text-text-primary">Details</h3>
		{#each list as [label, value] (label)}
			<div class="flex items-start gap-2 py-1">
				<!-- Wide enough for "Modified", so every value starts on one line. -->
				<span class="w-[72px] shrink-0 text-text-secondary">{label}</span>
				<span class="min-w-0 flex-1 break-words text-text-primary select-text">{value}</span>
			</div>
		{/each}
	</div>
{/snippet}

<aside
	class="h-full w-[260px] overflow-y-auto border-l border-border-secondary p-4 text-xs"
	aria-label="Details"
>
	{#if content.kind === 'single'}
		{@const item = content.item}
		{@const Icon = getFileIcon(item.name, item.isDir)}
		<div class="flex h-32 w-full items-center justify-center overflow-hidden rounded-md">
			{#if !item.isDir && getPreviewType(item.name) === 'image' && failedThumbnail !== item.path}
				<img
					src={getThumbnailUrl(item.path, 456, 256)}
					alt=""
					class="h-full w-full object-contain"
					onerror={() => (failedThumbnail = item.path)}
				/>
			{:else if item.isDir}
				<span class="text-folder"
					><FolderGlyph size={64} strokeWidth={1.2} {...appearance(item.path)} /></span
				>
			{:else}
				<Icon
					size={64}
					strokeWidth={1.2}
					class="text-text-secondary"
					style={settingsStore.current.fileIconTints
						? `color: ${getFileTint(item.name, false) ?? 'currentColor'}`
						: undefined}
				/>
			{/if}
		</div>
		<p class="m-0 mt-3 text-sm font-semibold break-words text-text-primary select-text">
			{item.name}
		</p>
		{@render details(rows(item))}
	{:else if content.kind === 'many'}
		<p class="m-0 text-sm font-semibold text-text-primary">
			{content.files + content.folders} items selected
		</p>
		{@render details([
			['Files', String(content.files)],
			['Folders', String(content.folders)],
			[
				'Size',
				content.folders > 0
					? `${formatFileSize(content.bytes)}, files only`
					: formatFileSize(content.bytes)
			]
		])}
		<div class="mt-[18px] flex flex-col gap-1 text-text-secondary">
			<h3 class="m-0 mb-0.5 text-[13px] font-medium text-text-primary">Selected</h3>
			{#each content.names as name, index (index)}
				<span class="truncate select-text">{name}</span>
			{/each}
			{#if content.more > 0}<span class="text-text-muted">and {content.more} more</span>{/if}
		</div>
	{:else}
		<p class="m-0 text-sm font-semibold break-words text-text-primary select-text">
			{folderName}
		</p>
		<p class="m-0 mt-0.5 text-text-secondary">{counted(content.count, 'item')}</p>
		<p class="m-0 mt-3.5 text-text-muted">Select an item to see its details.</p>
	{/if}
</aside>
