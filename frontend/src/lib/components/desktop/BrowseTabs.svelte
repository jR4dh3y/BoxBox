<script lang="ts">
	import type { BrowseTabState, BrowseTabsController } from '#lib/browse/tabs.svelte.js';

	interface Props {
		controller: BrowseTabsController;
		onTabChange?: () => void;
	}

	let { controller, onTabChange }: Props = $props();

	const tabButtonClass =
		'flex h-7 min-w-0 max-w-52 items-center gap-2 rounded px-2 text-left text-xs transition-colors';

	function tabTitle(tab: BrowseTabState): string {
		const path = tab.panes[0]?.location.path ?? '';
		return path ? (path.split('/').at(-1) ?? path) : 'This Server';
	}
</script>

<nav
	class="flex h-9 shrink-0 items-center gap-2 border-b border-border-secondary bg-surface-primary px-2"
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
					title={tab.panes[0]?.location.path || 'This Server'}
					onclick={() => {
						controller.activateTab(tab.id);
						onTabChange?.();
					}}
				>
					<span class="truncate">{title}</span>
				</button>
				<button
					type="button"
					class="mr-1 flex h-5 w-5 shrink-0 items-center justify-center rounded text-sm text-text-muted hover:bg-surface-tertiary hover:text-text-primary disabled:cursor-not-allowed disabled:opacity-40"
					aria-label={`Close ${title} tab`}
					title="Close tab"
					disabled={controller.tabs.length === 1}
					onclick={() => {
						controller.closeTab(tab.id);
						onTabChange?.();
					}}
				>
					×
				</button>
			</div>
		{/each}
	</div>

	<button
		type="button"
		class="flex h-7 w-7 shrink-0 items-center justify-center rounded text-base text-text-secondary hover:bg-surface-secondary hover:text-text-primary"
		aria-label="New tab"
		title="New tab (Ctrl+T)"
		onclick={() => {
			controller.newTab();
			onTabChange?.();
		}}
	>
		+
	</button>

	<div class="ml-auto shrink-0">
		<button
			type="button"
			class="h-7 rounded px-2 text-xs text-text-secondary hover:bg-surface-secondary hover:text-text-primary aria-pressed:bg-surface-elevated aria-pressed:text-text-primary"
			aria-pressed={controller.hasSplit}
			title="Toggle split pane (F3)"
			onclick={() => {
				controller.toggleSplit();
				onTabChange?.();
			}}
		>
			{controller.hasSplit ? 'Unsplit' : 'Split'}
		</button>
	</div>
</nav>
