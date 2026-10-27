<script lang="ts">
	import type { Snippet } from 'svelte';

	let { label, children }: { label: string; children: Snippet } = $props();
</script>

<div
	class="flex min-h-10 flex-wrap items-center justify-between gap-x-4 gap-y-2 border-b border-border-secondary py-1.5 text-[13px] last:border-b-0"
>
	<span class="text-text-primary">{label}</span>
	{@render children()}
</div>
