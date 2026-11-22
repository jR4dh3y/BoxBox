<script lang="ts">
	/** A folder's icon in its customised colour, with its emblem when it has one. */
	import { Folder } from 'lucide-svelte';

	let {
		size,
		color,
		emblem,
		emblemColor,
		strokeWidth = 2
	}: {
		size: number;
		color: string | null;
		emblem: string | null;
		emblemColor: string | null;
		strokeWidth?: number;
	} = $props();

	const tint = $derived(color ? `color: ${color}` : undefined);
</script>

<!-- The colour goes on the drawing itself: over a wallpaper, `.text-folder svg` sets its own. -->
<span class="relative inline-flex shrink-0" style:width="{size}px" style:height="{size}px">
	{#if emblem}
		{#await import('./EmblemedFolder.svelte')}
			<Folder {size} {strokeWidth} style={tint} />
		{:then { default: EmblemedFolder }}
			<EmblemedFolder {size} {strokeWidth} {emblem} {tint} color={emblemColor ?? color} />
		{/await}
	{:else}
		<Folder {size} {strokeWidth} style={tint} />
	{/if}
</span>
