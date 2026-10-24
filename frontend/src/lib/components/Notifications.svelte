<script lang="ts">
	import { toastStore } from '#lib/stores/toast.svelte.js';
	import LoadError from './LoadError.svelte';
</script>

{#if toastStore.toasts.length > 0}
	{#await import('./ui/Toast.svelte') then { default: Toast }}
		<Toast />
	{:catch}
		<LoadError label="notifications" />
	{/await}
{/if}
