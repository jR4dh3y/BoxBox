<script lang="ts">
	import Button from '#lib/components/ui/Button.svelte';

	/** Reload retries failed module downloads because browsers can cache failed imports. */
	let {
		label,
		onClose,
		inline = false
	}: { label: string; onClose?: () => void; inline?: boolean } = $props();
</script>

<div
	role="alert"
	class="{inline
		? 'm-4'
		: 'fixed top-4 left-1/2 z-[110] -translate-x-1/2'} flex items-center gap-3 rounded-lg border border-border-primary bg-surface-elevated px-4 py-3 text-sm shadow-lg"
>
	<span>Could not load the {label}. Check your connection, then reload.</span>
	<Button size="sm" onclick={() => window.location.reload()}>Reload</Button>
	{#if onClose}
		<Button size="sm" variant="secondary" onclick={onClose}>Close</Button>
	{/if}
</div>
