<script lang="ts">
	/** A folder's icon in its customised colour, with its emblem inside the body. */
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

<span class="relative inline-flex shrink-0" style:width="{size}px" style:height="{size}px">
	<Folder {size} {strokeWidth} style={color ? `color: ${color}` : undefined} />
	{#if emblem}
		<!-- As the desktop draws a small folder's emblem: inside the body, where it stays legible. -->
		<span class="absolute flex" style:left="{size * 0.3}px" style:top="{size * 0.36}px">
			<!-- Emblems are uncommon, so their drawing loads with the first one. -->
			{#await import('./EmblemIcon.svelte') then { default: EmblemIcon }}
				<EmblemIcon name={emblem} size={size * 0.4} color={emblemColor ?? color} />
			{/await}
		</span>
	{/if}
</span>
