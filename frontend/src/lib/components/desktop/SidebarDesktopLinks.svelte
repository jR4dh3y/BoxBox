<script lang="ts">
	import { Network, Trash2, Usb } from 'lucide-svelte';
	import type { BrowsePage } from '#lib/browse/location.svelte.js';
	import { locationTitle } from '#lib/browse/locationTitle.js';

	let {
		pages,
		currentPage,
		onOpenPage,
		itemClass,
		activeClass
	}: {
		pages: Exclude<BrowsePage, 'files'>[];
		currentPage: BrowsePage;
		onOpenPage: (page: BrowsePage) => void;
		itemClass: string;
		activeClass: string;
	} = $props();

	const ICONS = { trash: Trash2, drives: Usb, servers: Network };
</script>

{#each pages as page (page)}
	{@const Icon = ICONS[page]}
	<button
		type="button"
		class="{itemClass} {currentPage === page ? activeClass : ''}"
		aria-current={currentPage === page ? 'page' : undefined}
		onclick={() => onOpenPage(page)}
	>
		<Icon size={16} class="shrink-0 opacity-80" />
		<span class="flex-1 truncate">{locationTitle({ page, path: '' })}</span>
	</button>
{/each}
