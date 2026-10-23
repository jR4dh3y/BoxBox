<script lang="ts">
	import { Plus, X } from 'lucide-svelte';
	import type { BrowseTabState, BrowseTabsController } from '#lib/browse/tabs.svelte.js';
	import { locationTitle } from '#lib/browse/locationTitle.js';

	interface Props {
		controller: BrowseTabsController;
		onTabChange?: () => void;
	}

	let { controller, onTabChange }: Props = $props();

	const tabButtonClass =
		'flex h-7 min-w-0 max-w-52 items-center gap-2 rounded px-2 text-left text-xs transition-colors';

	function tabTitle(tab: BrowseTabState): string {
		const location = tab.panes[0]?.location;
		if (!location) return '';
		if (location.page !== 'files' || !location.path) return locationTitle(location);
		return location.path.split('/').at(-1) ?? location.path;
	}
</script>

<nav
	class="flex h-8 shrink-0 items-center gap-1 border-b border-border-secondary bg-surface-primary px-2"
	aria-label="Browse tabs"
>
	<div class="flex min-w-0 items-center gap-1 overflow-hidden">
		{#each controller.tabs as tab (tab.id)}
			{@const active = tab.id === controller.activeTabId}
			{@const title = tabTitle(tab)}
			<div class="group flex min-w-0 items-center rounded {active ? 'bg-surface-elevated' : ''}">
				<button
					type="button"
					class="{tabButtonClass} {active
						? 'text-text-primary'
						: 'text-text-secondary hover:bg-surface-secondary hover:text-text-primary'}"
					aria-current={active ? 'page' : undefined}
					title={tab.panes[0]?.location.path || title}
					onclick={() => {
						controller.activateTab(tab.id);
						onTabChange?.();
					}}
				>
					<span class="truncate">{title}</span>
				</button>
				<button
					type="button"
					class="mr-1 flex h-5 w-5 shrink-0 items-center justify-center rounded text-text-muted opacity-0 group-hover:opacity-100 hover:bg-surface-tertiary hover:text-text-primary focus-visible:opacity-100 {active
						? 'opacity-100'
						: ''}"
					aria-label={`Close ${title} tab`}
					title="Close tab (Ctrl+W)"
					onclick={() => {
						controller.closeTab(tab.id);
						onTabChange?.();
					}}
				>
					<X size={12} />
				</button>
			</div>
		{/each}
	</div>

	<button
		type="button"
		class="flex h-7 w-7 shrink-0 items-center justify-center rounded text-text-secondary hover:bg-surface-secondary hover:text-text-primary"
		aria-label="New tab"
		title="New tab (Ctrl+T)"
		onclick={() => {
			controller.newTab();
			onTabChange?.();
		}}
	>
		<Plus size={14} />
	</button>
</nav>
