<script lang="ts">
	// Development only. The import is dropped from production builds, so the
	// gallery adds nothing to the JavaScript the app ships.
	const gallery = import.meta.env.DEV ? import('./Gallery.svelte') : undefined;
</script>

{#if gallery}
	{#await gallery then { default: Gallery }}
		<Gallery />
	{/await}
{/if}
