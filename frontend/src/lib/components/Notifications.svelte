<script lang="ts">
	import type { Snippet } from 'svelte';
	import { toastStore } from '#lib/stores/toast.svelte.js';
	import LoadError from './LoadError.svelte';

	/** Pages add their own panels below the toasts, so every notice stacks in one column. */
	let { children }: { children?: Snippet } = $props();
</script>

<div
	class="pointer-events-none fixed right-4 bottom-11 z-[100] flex w-[380px] max-w-[calc(100vw-2rem)] flex-col gap-2"
>
	{#if toastStore.toasts.length > 0}
		{#await import('./ui/Toast.svelte') then { default: Toast }}
			<Toast />
		{:catch}
			<LoadError label="notifications" />
		{/await}
	{/if}
	{@render children?.()}
</div>
