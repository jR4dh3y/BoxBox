<script lang="ts">
	/**
	 * StatusBar component - bottom status bar with item count and view options
	 */
	import { List, LayoutGrid } from 'lucide-svelte';

	interface Props {
		itemCount?: number;
		selectedCount?: number;
		viewMode?: 'list' | 'grid';
		onViewModeChange?: (mode: 'list' | 'grid') => void;
	}

	let { itemCount = 0, selectedCount = 0, viewMode = 'list', onViewModeChange }: Props = $props();

	const statusText = $derived.by(() => {
		if (selectedCount > 0) {
			return `${selectedCount} of ${itemCount} selected`;
		}
		return `${itemCount} item${itemCount !== 1 ? 's' : ''}`;
	});
</script>

<footer
	class="flex items-center justify-between border-t border-border-secondary bg-surface-primary px-3 py-1 text-xs text-text-secondary"
>
	<div class="flex items-center gap-3">
		<span>{statusText}</span>
	</div>
	<div class="flex items-center gap-3">
		<div class="flex gap-0.5 rounded bg-surface-secondary p-0.5">
			<button
				type="button"
				class="flex h-5.5 w-5.5 cursor-pointer items-center justify-center rounded-sm border-none bg-transparent text-text-muted transition-all duration-100 hover:text-text-secondary {viewMode ===
				'list'
					? 'bg-surface-elevated text-text-primary'
					: ''}"
				onclick={() => onViewModeChange?.('list')}
				title="List view"
			>
				<List size={14} />
			</button>
			<button
				type="button"
				class="flex h-5.5 w-5.5 cursor-pointer items-center justify-center rounded-sm border-none bg-transparent text-text-muted transition-all duration-100 hover:text-text-secondary {viewMode ===
				'grid'
					? 'bg-surface-elevated text-text-primary'
					: ''}"
				onclick={() => onViewModeChange?.('grid')}
				title="Grid view"
			>
				<LayoutGrid size={14} />
			</button>
		</div>
	</div>
</footer>
