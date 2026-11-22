<script lang="ts">
	/**
	 * A folder with its emblem: inside the body on a small icon, where it stays legible, and over
	 * the outline's bottom left corner on a large one, as the desktop app draws them (`emblemed` in
	 * `desktop/src/components/file_list.rs`). Emblems are uncommon, so this loads with the first.
	 */
	import { Folder } from 'lucide-svelte';
	import EmblemIcon from './EmblemIcon.svelte';

	/** From this size the emblem moves to the corner. */
	const CORNER_EMBLEM = 40;
	/** The corner emblem's edge, against the folder's. */
	const EMBLEM_SCALE = 0.36;

	let {
		size,
		strokeWidth,
		emblem,
		tint,
		color
	}: {
		size: number;
		strokeWidth: number;
		emblem: string;
		/** The folder's own colour, as a style. */
		tint: string | undefined;
		/** The emblem's. */
		color: string | null;
	} = $props();

	const corner = $derived(size >= CORNER_EMBLEM);
	// In the folder's 24 units its outline's outer edge is at x 1.15 and y 20.85; the emblem's
	// glyph fills 20 of its own 24, so its padding is a twelfth of its edge.
	const mark = $derived.by(() => {
		if (!corner) return { left: size * 0.3, top: size * 0.36, edge: size * 0.4 };
		const unit = size / 24;
		const edge = size * EMBLEM_SCALE;
		const pad = edge / 12;
		return { left: 1.15 * unit - pad, top: 20.85 * unit - edge + pad, edge };
	});
</script>

{#if corner}
	<!-- Lucide's folder, open at the bottom left where the emblem sits. -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width={strokeWidth}
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
		style={tint}
	>
		<path
			d="M10.8 20H20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v6.2"
		/>
	</svg>
{:else}
	<Folder {size} {strokeWidth} style={tint} />
{/if}
<span class="absolute flex" style:left="{mark.left}px" style:top="{mark.top}px">
	<EmblemIcon name={emblem} size={mark.edge} {color} />
</span>
