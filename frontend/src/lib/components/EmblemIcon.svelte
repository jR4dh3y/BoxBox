<script lang="ts">
	/** A folder emblem: a special folder's from the bundle, any other from the lazy catalogue. */
	import {
		Download,
		FileText,
		House,
		Image,
		LayoutTemplate,
		Monitor,
		Music,
		Share2,
		Video
	} from 'lucide-svelte';
	import EmblemShapes from './EmblemShapes.svelte';

	let { name, size, color }: { name: string; size: number; color?: string | null } = $props();

	const BUNDLED: Record<string, typeof House> = {
		house: House,
		monitor: Monitor,
		'file-text': FileText,
		download: Download,
		music: Music,
		image: Image,
		video: Video,
		'layout-template': LayoutTemplate,
		'share-2': Share2
	};
	const Bundled = $derived(BUNDLED[name]);
</script>

{#if Bundled}
	<Bundled {size} strokeWidth={2} style={color ? `color: ${color}` : undefined} />
{:else}
	<!-- A chosen emblem loads the catalogue; special folders never need it. -->
	{#await import('#lib/browse/emblems.js').then((emblems) => emblems.loadEmblems()) then catalog}
		{@const shapes = catalog.shapes(name)}
		{#if shapes}<EmblemShapes {shapes} {size} {color} />{/if}
	{/await}
{/if}
