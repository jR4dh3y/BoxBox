<script module lang="ts">
	export type ContextMenuItem<Id extends string = string> =
		| 'separator'
		| {
				id: Id;
				label: string;
				icon?: typeof import('lucide-svelte').Copy;
				disabled?: boolean;
				shortcut?: string;
		  };
</script>

<script lang="ts" generics="Id extends string">
	/** Reusable right-click menu. */
	interface Props {
		items: ContextMenuItem<Id>[];
		x: number;
		y: number;
		onSelect: (id: Id) => void;
		onClose: () => void;
	}

	let { items, x, y, onSelect, onClose }: Props = $props();

	let menuRef: HTMLDivElement | undefined = $state();

	// Adjust position to keep menu within viewport
	let adjustedPosition = $derived.by(() => {
		if (!menuRef) return { x, y };

		const menuWidth = 200; // approximate width
		const menuHeight = items.length * 36; // approximate height
		const viewportWidth = typeof window !== 'undefined' ? window.innerWidth : 1920;
		const viewportHeight = typeof window !== 'undefined' ? window.innerHeight : 1080;

		let adjustedX = x;
		let adjustedY = y;

		if (x + menuWidth > viewportWidth) {
			adjustedX = viewportWidth - menuWidth - 8;
		}
		if (y + menuHeight > viewportHeight) {
			adjustedY = viewportHeight - menuHeight - 8;
		}

		return { x: adjustedX, y: adjustedY };
	});

	function handleItemClick(item: Exclude<ContextMenuItem<Id>, 'separator'>) {
		if (item.disabled) return;
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

<svelte:window onkeydown={handleKeyDown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
	class="fixed inset-0 z-50"
	onclick={handleBackdropClick}
	oncontextmenu={(e) => {
		e.preventDefault();
		onClose();
	}}
>
	<div
		bind:this={menuRef}
		class="fixed max-w-70 min-w-45 rounded-lg border border-border-primary bg-surface-context-menu py-1 shadow-xl"
		style="left: {adjustedPosition.x}px; top: {adjustedPosition.y}px;"
		role="menu"
	>
		{#each items as item, index (item === 'separator' ? `separator-${index}` : item.id)}
			{#if item === 'separator'}
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
