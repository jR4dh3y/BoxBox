<script lang="ts">
	/** An emblem's shapes as a Lucide icon draws them. */
	import type { EmblemShape } from '#lib/browse/emblems.js';

	let {
		shapes,
		size,
		color = null
	}: { shapes: readonly EmblemShape[]; size: number; color?: string | null } = $props();
</script>

<svg
	xmlns="http://www.w3.org/2000/svg"
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="2"
	stroke-linecap="round"
	stroke-linejoin="round"
	style:color
	aria-hidden="true"
>
	{#each shapes as shape, index (index)}
		<svelte:element this={shape.tag} {...shape.attrs} />
	{/each}
</svg>
