<script lang="ts">
	/** A grid tile's name: two whole lines, the second shortened in its middle. */
	import { tileLabelLines, type Measure } from '#lib/browse/tileLabel.js';

	let {
		name,
		width,
		measure
	}: {
		name: string;
		/** The label's width in pixels. */
		width: number;
		/** Null until the page's font is known; the name is then clamped by CSS. */
		measure: Measure | null;
	} = $props();

	const lines = $derived(measure ? tileLabelLines(name, width, measure) : null);
</script>

{#if lines}
	<span class="block truncate">{lines[0]}</span>
	{#if lines[1] !== null}<span class="block truncate">{lines[1]}</span>{/if}
{:else}
	<span class="line-clamp-2 break-words">{name}</span>
{/if}
