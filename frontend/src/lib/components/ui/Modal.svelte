<script lang="ts">
	import type { Snippet } from 'svelte';
	import { X } from 'lucide-svelte';

	interface Props {
		open?: boolean;
		title?: string;
		size?: 'md' | 'lg';
		children: Snippet;
		headerActions?: Snippet;
		footer?: Snippet;
		onclose?: () => void;
	}

	let {
		open = false,
		title,
		size = 'md',
		children,
		headerActions,
		footer,
		onclose
	}: Props = $props();

	const widthClass = $derived(size === 'lg' ? 'max-w-3xl' : 'max-w-md');

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onclose?.();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onclose?.();
		}
	}

	let dialogEl = $state<HTMLDivElement | undefined>();

	$effect(() => {
		if (open) {
			// Focus into the dialog so Escape/typing reach it immediately
			// (otherwise a keyboard opener like F2 leaves focus on the page behind).
			(dialogEl?.querySelector<HTMLElement>('input') ?? dialogEl)?.focus();
		}
	});
</script>

{#if open}
	<div
		bind:this={dialogEl}
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
		role="dialog"
		aria-modal="true"
		tabindex="-1"
		onclick={handleBackdropClick}
		onkeydown={handleKeydown}
	>
		<div
			class="mx-4 flex max-h-[90vh] w-full {widthClass} flex-col overflow-hidden rounded-lg border border-border-primary bg-surface-primary shadow-xl"
		>
			{#if title}
				<div class="flex items-center justify-between border-b border-border-secondary px-4 py-3">
					<h2 class="text-lg font-medium text-text-primary">{title}</h2>
					<div class="flex items-center gap-2">
						{@render headerActions?.()}
						<button
							type="button"
							class="rounded p-1 text-text-secondary transition-colors hover:text-text-primary"
							onclick={onclose}
							aria-label="Close"
						>
							<X size={18} />
						</button>
					</div>
				</div>
			{/if}
			<div class="overflow-y-auto p-4">
				{@render children()}
			</div>
			{#if footer}
				<div class="flex justify-end gap-2 border-t border-border-secondary px-4 py-3">
					{@render footer()}
				</div>
			{/if}
		</div>
	</div>
{/if}
