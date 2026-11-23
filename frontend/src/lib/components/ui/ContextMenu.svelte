<script lang="ts">
	/**
	 * Context Menu component - reusable right-click menu
	 * Follows UI component patterns from contributing guidelines
	 */
	export interface ContextMenuItem {
		id: string;
		label: string;
		icon?: typeof import('lucide-svelte').Copy;
		disabled?: boolean;
		separator?: boolean;
		shortcut?: string;
	}

	interface Props {
		items: ContextMenuItem[];
		aboveSidebar?: boolean;
		x: number;
		y: number;
		onSelect: (id: string) => void;
		onClose: () => void;
	}

	let { items, x, y, onSelect, onClose, aboveSidebar = false }: Props = $props();

	let menuWidth = $state(0);
	let menuHeight = $state(0);
	let viewportWidth = $state(0);
	let viewportHeight = $state(0);
	let adjustedX = $derived(Math.max(8, Math.min(x, viewportWidth - menuWidth - 8)));
	let adjustedY = $derived(Math.max(8, Math.min(y, viewportHeight - menuHeight - 8)));

	function handleItemClick(item: ContextMenuItem) {
		if (item.disabled || item.separator) return;
		onSelect(item.id);
		onClose();
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			onClose();
		}
	}

	function handleBackdropClick(event: MouseEvent) {
		if (event.target === event.currentTarget) {
			onClose();
		}
	}
</script>

<svelte:window
	bind:innerWidth={viewportWidth}
	bind:innerHeight={viewportHeight}
	onkeydown={handleKeyDown}
/>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
	class={['fixed inset-0', aboveSidebar ? 'z-[65]' : 'z-50']}
	onclick={handleBackdropClick}
	oncontextmenu={(e) => {
		e.preventDefault();
		onClose();
	}}
>
	<div
		bind:offsetWidth={menuWidth}
		bind:offsetHeight={menuHeight}
		class="fixed max-h-[calc(100dvh-16px)] w-max max-w-[min(17.5rem,calc(100dvw-16px))] origin-top-left overflow-y-auto overscroll-contain rounded-lg border border-border-primary bg-surface-context-menu py-1 shadow-xl motion-safe:animate-pop-in sm:min-w-45"
		style:left={`${adjustedX}px`}
		style:top={`${adjustedY}px`}
		role="menu"
	>
		{#each items as item (item.id)}
			{#if item.separator}
				<div class="mx-2 my-1 h-px bg-border-secondary"></div>
			{:else}
				<button
					type="button"
					class="flex w-full items-center gap-3 px-3 py-2 text-left text-[13px] transition-colors
						{item.disabled
						? 'cursor-not-allowed text-text-disabled'
						: 'cursor-pointer text-text-primary hover:bg-surface-secondary'}"
					disabled={item.disabled}
					onclick={() => handleItemClick(item)}
					role="menuitem"
				>
					{#if item.icon}
						{@const IconComponent = item.icon}
						<span class="flex h-4 w-4 items-center justify-center text-text-secondary">
							<IconComponent size={14} />
						</span>
					{:else}
						<span class="w-4"></span>
					{/if}
					<span class="flex-1">{item.label}</span>
					{#if item.shortcut}
						<span class="text-[11px] text-text-muted">{item.shortcut}</span>
					{/if}
				</button>
			{/if}
		{/each}
	</div>
</div>
