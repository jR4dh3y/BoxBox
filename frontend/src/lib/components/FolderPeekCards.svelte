<script lang="ts">
	/** A folder's items stacked inside its glyph, front first, as the desktop app draws them. */
	import { getThumbnailUrl, type FileInfo } from '#lib/api/files.js';
	import { getPreviewType } from '#lib/utils/fileTypes.js';

	let { items, size }: { items: readonly FileInfo[]; size: number } = $props();

	const width = $derived(size * 0.4);
	const height = $derived(size * 0.3);
	const step = $derived(size * 0.055);
	let failed = $state<ReadonlySet<string>>(new Set());
</script>

<!-- The folder's body, below its tab, in the 24-unit icon: x 2-22, y 6-20. -->
{#each [...items].reverse() as item, reversed (item.path)}
	{@const depth = items.length - 1 - reversed - (items.length - 1) / 2}
	{@const picture = !item.isDir && getPreviewType(item.name) === 'image' && !failed.has(item.path)}
	<span
		class="absolute overflow-hidden rounded-[2px] border"
		style:left="{size * 0.5 - width / 2 + depth * step}px"
		style:top="{size * 0.56 - height / 2 - depth * step * 0.8}px"
		style:width="{width}px"
		style:height="{height}px"
		style:border-color="color-mix(in srgb, currentColor {picture ? 35 : 45}%, transparent)"
		style:background-color={picture ? null : 'color-mix(in srgb, currentColor 8%, transparent)'}
	>
		{#if picture}
			<img
				src={getThumbnailUrl(item.path, 64, 48)}
				alt=""
				loading="lazy"
				class="h-full w-full object-cover"
				onerror={() => (failed = new Set([...failed, item.path]))}
			/>
		{/if}
	</span>
{/each}
