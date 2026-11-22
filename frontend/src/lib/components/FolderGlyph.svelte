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
</script>

<span
	class="relative inline-flex shrink-0"
	style:width="{size}px"
	style:height="{size}px"
	style:color={color ?? undefined}
>
	{#if emblem}
		{#await import('./EmblemedFolder.svelte')}
			<Folder {size} {strokeWidth} />
		{:then { default: EmblemedFolder }}
			<EmblemedFolder {size} {strokeWidth} {emblem} color={emblemColor ?? color} />
		{/await}
	{:else}
		<Folder {size} {strokeWidth} />
	{/if}
</span>
