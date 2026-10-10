<script lang="ts" generics="P extends Record<string, unknown>">
	import type { Component } from 'svelte';
	import { toastStore } from '$lib/stores/toast.svelte';

	/**
	 * Downloads an overlay on first use. If the download fails it tells the user and calls
	 * `onFailed`, which should close the request so it can be started again.
	 */
	let {
		load,
		props,
		onFailed
	}: {
		load: () => Promise<{ default: Component<P> }>;
		props: P;
		onFailed?: () => void;
	} = $props();

	async function download() {
		try {
			return await load();
		} catch {
			toastStore.error('Could not load this window. Check your connection and reload the page.');
			onFailed?.();
			return null;
		}
	}
</script>

{#await download() then module}
	{#if module}
		<module.default {...props} />
	{/if}
{/await}
