<script lang="ts">
	import { isLocalDesktop } from '#lib/api/desktop.js';
	import LoadError from '#lib/components/LoadError.svelte';
</script>

<div class="h-screen">
	{#if isLocalDesktop()}
		{#await import('#lib/components/desktop/BrowseWorkspace.svelte') then { default: BrowseWorkspace }}
			<BrowseWorkspace />
		{:catch}<LoadError label="desktop workspace" inline />{/await}
	{:else}{#await import('#lib/components/BrowseBrowser.svelte') then { default: BrowseBrowser }}<BrowseBrowser
			/>{:catch}<LoadError label="file browser" inline />{/await}{/if}
</div>
