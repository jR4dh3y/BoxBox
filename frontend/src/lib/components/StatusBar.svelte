<script lang="ts">
	/**
	 * StatusBar component - bottom status bar with item count and view options
	 */
	import { List, LayoutGrid } from 'lucide-svelte';
	import type { ViewMode } from '$lib/types/files';

	interface Props {
		itemCount?: number;
		totalCount?: number;
		selectedCount?: number;
		viewMode?: ViewMode;
		hasMore?: boolean;
		isLoadingMore?: boolean;
		onLoadMore?: () => void;
		onViewModeChange?: (mode: ViewMode) => void;
	}

	let {
		itemCount = 0,
		totalCount,
		selectedCount = 0,
		viewMode = 'list',
		hasMore = false,
		isLoadingMore = false,
		onLoadMore,
		onViewModeChange
	}: Props = $props();

	const statusText = $derived.by(() => {
		if (selectedCount > 0) {
			return `${selectedCount} of ${itemCount} selected`;
		}

		if (totalCount !== undefined && totalCount > itemCount) {
			return `${itemCount} of ${totalCount} items`;
		}

		return `${itemCount} item${itemCount !== 1 ? 's' : ''}`;
	});

	const viewButtonClass =
		'flex h-5.5 w-5.5 cursor-pointer items-center justify-center rounded-sm border-none bg-transparent text-text-muted transition-all duration-100 hover:text-text-secondary';
	const activeViewButtonClass = 'bg-surface-elevated text-text-primary';
</script>

<footer
	class="flex items-center justify-between border-t border-border-secondary bg-surface-primary px-3 py-1 text-xs text-text-secondary"
>
	<div class="flex items-center gap-3">
		<span>{statusText}</span>
		{#if hasMore}
			<button
				type="button"
				class="rounded border border-border-primary bg-surface-secondary px-2 py-0.5 text-xs text-text-secondary transition-colors hover:bg-surface-tertiary hover:text-text-primary disabled:cursor-not-allowed disabled:opacity-60"
				onclick={onLoadMore}
				disabled={isLoadingMore}
			>
				{isLoadingMore ? 'Loading...' : 'Load more'}
			</button>
		{/if}
	</div>
	<div class="flex items-center gap-3">
		<div class="flex gap-0.5 rounded bg-surface-secondary p-0.5">
			<button
				type="button"
				class="{viewButtonClass} {viewMode === 'list' ? activeViewButtonClass : ''}"
				onclick={() => onViewModeChange?.('list')}
				title="List view"
				aria-label="List view"
				aria-pressed={viewMode === 'list'}
			>
				<List size={14} />
			</button>
			<button
				type="button"
				class="{viewButtonClass} {viewMode === 'grid' ? activeViewButtonClass : ''}"
				onclick={() => onViewModeChange?.('grid')}
				title="Grid view"
				aria-label="Grid view"
				aria-pressed={viewMode === 'grid'}
			>
				<LayoutGrid size={14} />
			</button>
		</div>
	</div>
</footer>
