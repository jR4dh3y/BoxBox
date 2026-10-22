<script lang="ts">
	import type { Snippet } from 'svelte';
	import { X } from 'lucide-svelte';

	interface Props {
		open?: boolean;
		title?: string;
		size?: 'md' | 'lg' | 'xl';
		/** Content that manages its own spacing and scrolling, like a two-pane layout, sets this false. */
		padded?: boolean;
		children: Snippet;
		headerActions?: Snippet;
		footer?: Snippet;
		onclose?: () => void;
		busy?: boolean;
	}

	let {
		open = false,
		title,
		size = 'md',
		padded = true,
		children,
		headerActions,
		footer,
		onclose,
		busy = false
	}: Props = $props();

	const widthClass = $derived(
		size === 'xl' ? 'max-w-2xl' : size === 'lg' ? 'max-w-3xl' : 'max-w-md'
	);

	function handleBackdropClick(e: MouseEvent) {
		if (!busy && e.target === e.currentTarget) {
			onclose?.();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			// A dialog opened from inside another one closes alone.
			e.stopPropagation();
			onclose?.();
		}
	}

	let dialogEl = $state<HTMLDialogElement | undefined>();

	$effect(() => {
		if (!open) return;
		const opener = document.activeElement;
		// Focus into the dialog so Escape/typing reach it immediately
		// (otherwise a keyboard opener like F2 leaves focus on the page behind).
		(dialogEl?.querySelector<HTMLElement>('input') ?? dialogEl)?.focus();
		return () => {
			if (opener instanceof HTMLElement && opener.isConnected)
				opener.focus({ preventScroll: true });
		};
	});
</script>

{#if open}
	<dialog
		bind:this={dialogEl}
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/55"
		role="dialog"
		aria-modal="true"
		aria-labelledby={title ? titleId : undefined}
		aria-label={title ? undefined : 'Dialog'}
		aria-busy={busy}
		tabindex="-1"
		onclick={handleBackdropClick}
		onkeydown={handleKeydown}
	>
		<div
			class="mx-4 flex max-h-[90vh] w-full {widthClass} flex-col overflow-hidden rounded-lg border border-border-primary bg-surface-secondary shadow-2xl"
		>
			{#if title}
				<div
					class="flex h-11 shrink-0 items-center justify-between gap-3 border-b border-border-secondary pr-2 pl-4"
				>
					<h2 class="min-w-0 truncate text-sm font-medium text-text-primary" {title}>{title}</h2>
					<div class="flex shrink-0 items-center gap-2">
						{@render headerActions?.()}
						<button
							type="button"
							class="flex h-7 w-7 items-center justify-center rounded text-text-secondary transition-colors hover:bg-surface-elevated hover:text-text-primary"
							onclick={onclose}
							disabled={busy}
							aria-label="Close"
						>
							<X size={16} />
						</button>
					</div>
				</div>
			{/if}
			<div class="min-h-0 flex-1 overflow-y-auto {padded ? 'p-4' : ''}">
				{@render children()}
			</div>
			{#if footer}
				<div class="flex justify-end gap-2 border-t border-border-secondary px-4 py-3">
					{@render footer()}
				</div>
			{/if}
		</div>
	</dialog>
{/if}
