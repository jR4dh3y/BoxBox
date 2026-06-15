<script lang="ts">
	/**
	 * Settings row component for individual settings
	 * Displays label, description, and a control (toggle, select, or custom)
	 */
	import type { Snippet } from 'svelte';

	interface Props {
		label: string;
		description?: string;
		children?: Snippet;
	}

	let { label, description, children }: Props = $props();
</script>

<div class="flex items-center justify-between py-2 px-3">
	<div class="flex flex-col gap-0.5">
		<span class="text-[13px] text-text-primary">{label}</span>
		{#if description}
			<span class="text-[11px] text-text-muted">{description}</span>
		{/if}
	</div>
	{@render children?.()}
</div>
