<script lang="ts">
	/**
	 * Breadcrumb component for path navigation
	 */
	import { Home, ChevronRight } from 'lucide-svelte';

	interface Props {
		segments: string[];
		onNavigate?: (path: string) => void;
	}

	let { segments = [], onNavigate }: Props = $props();

	function buildPath(index: number): string {
		return segments.slice(0, index + 1).join('/');
	}

	function handleClick(index: number): void {
		onNavigate?.(buildPath(index));
	}

	function handleRootClick(): void {
		onNavigate?.('');
	}
</script>

<nav aria-label="Breadcrumb" class="flex items-center gap-1 text-sm">
	<button
		type="button"
		onclick={handleRootClick}
		class="flex items-center text-text-secondary transition-colors hover:text-text-primary"
		aria-label="Go to root"
	>
		<Home size={18} />
	</button>

	{#if segments.length > 0}
		{#each segments as segment, index (index)}
			<ChevronRight size={16} class="shrink-0 text-text-muted" />

			{#if index === segments.length - 1}
				<span
					class="max-w-[200px] truncate font-medium text-text-primary"
					title={segment}
					aria-current="page"
				>
					{segment}
				</span>
			{:else}
				<button
					type="button"
					onclick={() => handleClick(index)}
					class="max-w-[150px] truncate text-text-secondary transition-colors hover:text-text-primary"
					title={segment}
				>
					{segment}
				</button>
			{/if}
		{/each}
	{/if}
</nav>
