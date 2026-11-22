<script lang="ts">
	/** What a rested-on row or tile is, as the desktop app's hover card shows it. */
	import { getThumbnailUrl } from '#lib/api/files.js';
	import { fullFileDate } from '#lib/browse/details.js';
	import type { FolderAppearance } from '#lib/browse/folderLooks.js';
	import {
		cardPlacement,
		describeTotals,
		type HoverCardState
	} from '#lib/browse/hoverCard.svelte.js';
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
		card,
		appearance
	}: {
		card: Extract<HoverCardState, { kind: 'shown' }>;
		appearance: (path: string) => FolderAppearance;
	} = $props();

	const WIDTH = 280;
	const item = $derived(card.item);
	const kind = $derived(item.isDir ? 'Folder' : getFileTypeDescription(item.name));
	const Icon = $derived(getFileIcon(item.name, item.isDir));
	const thumbnail = $derived(!item.isDir && getPreviewType(item.name) === 'image');
	let thumbnailFailed = $state(false);
	let height = $state(0);
	const place = $derived(
		cardPlacement(card, WIDTH, height, { width: window.innerWidth, height: window.innerHeight })
	);
	const rows = $derived.by(() => {
		const list: [string, string][] = [];
		const contents = card.contents;
		if (contents === null) list.push(['Size', formatFileSize(item.size)]);
		else if (contents.kind === 'loading') list.push(['Contains', 'Counting…']);
		else if (contents.kind === 'failed') list.push(['Contains', 'Could not read it']);
		else {
			list.push(['Contains', describeTotals(contents.totals)]);
			list.push(['Size', formatFileSize(contents.totals.bytes)]);
		}
		list.push(['Modified', fullFileDate(item.modTime)]);
		if (card.location !== null) list.push(['Location', card.location || '/']);
		return list;
	});
</script>

<div
	class="hover-card pointer-events-none fixed z-[60] flex flex-col gap-2.5 rounded-lg border border-border-primary bg-surface-elevated p-3 text-xs shadow-lg"
	style:width="{WIDTH}px"
	style:left="{place.left}px"
	style:top="{place.top}px"
	bind:clientHeight={height}
	role="tooltip"
>
	<div class="flex items-center gap-2.5">
		<div
			class="relative flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded"
		>
			{#if thumbnail && !thumbnailFailed}
				<img
					src={getThumbnailUrl(item.path, 80, 80)}
					alt=""
					class="h-full w-full object-cover"
					onerror={() => (thumbnailFailed = true)}
				/>
			{:else if item.isDir}
				<span class="text-folder"
					><FolderGlyph size={36} strokeWidth={1.5} {...appearance(item.path)} /></span
				>
			{:else}
				<Icon
					size={36}
					strokeWidth={1.5}
					class="text-text-secondary"
					style={settingsStore.current.fileIconTints
						? `color: ${getFileTint(item.name, false) ?? 'currentColor'}`
						: undefined}
				/>
			{/if}
		</div>
		<div class="flex min-w-0 flex-col">
			<span class="truncate text-sm text-text-primary">{item.name}</span>
			<span class="truncate text-text-secondary">{kind}</span>
		</div>
	</div>
	<div class="h-px bg-border-secondary"></div>
	<div class="flex flex-col gap-1">
		{#each rows as [label, value] (label)}
			<div class="flex items-start gap-2">
				<!-- Wide enough for every label, so the values start in one column. -->
				<span class="w-[76px] shrink-0 text-text-muted">{label}</span>
				<span class="min-w-0 flex-1 break-words text-text-primary">{value}</span>
			</div>
		{/each}
	</div>
</div>

<style>
	.hover-card {
		animation: hover-card-enter 140ms cubic-bezier(0.2, 0, 0, 1);
	}
	@keyframes hover-card-enter {
		from {
			opacity: 0;
			transform: translateY(-4px);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.hover-card {
			animation: none;
		}
	}
</style>
