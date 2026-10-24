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
		busy?: boolean;
	}

	let {
		open = false,
		title,
		size = 'md',
		children,
		headerActions,
		footer,
		onclose,
		busy = false
	}: Props = $props();

	const widthClass = $derived(size === 'lg' ? 'max-w-3xl' : 'max-w-md');
	const titleId = $props.id();

	function handleBackdropClick(e: MouseEvent) {
		if (!busy && e.target === e.currentTarget) {
			onclose?.();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			if (!busy) onclose?.();
		}
		if (e.key === 'Tab' && dialogEl) {
			const controls = Array.from(
				dialogEl.querySelectorAll<HTMLElement>(
					'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex="0"]'
				)
			).filter((element) => element.getClientRects().length > 0);
			const first = controls[0];
			const last = controls.at(-1);
			if (first && last && e.shiftKey && document.activeElement === first) {
				e.preventDefault();
				last.focus();
			} else if (first && last && !e.shiftKey && document.activeElement === last) {
				e.preventDefault();
				first.focus();
			}
		}
	}

	let dialogEl = $state<HTMLDialogElement | undefined>();

	$effect(() => {
		if (open && dialogEl && !dialogEl.open) {
			const dialog = dialogEl;
			const opener = document.activeElement;
			dialog.showModal();
			dialog.querySelector<HTMLElement>('input:not(:disabled)')?.focus();
			return () => {
				dialog.close();
				if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
			};
		}
	});
</script>

{#if open}
	<dialog
		bind:this={dialogEl}
		class="fixed inset-0 z-50 m-0 flex h-dvh max-h-none w-screen max-w-none items-center justify-center border-0 bg-transparent p-0 text-text-primary backdrop:bg-black/50"
		aria-modal="true"
		aria-labelledby={title ? titleId : undefined}
		aria-label={title ? undefined : 'Dialog'}
		aria-busy={busy}
		tabindex="-1"
		onclick={handleBackdropClick}
		onkeydown={handleKeydown}
	>
		<div
			class="mx-4 flex max-h-[90vh] w-full {widthClass} flex-col overflow-hidden rounded-lg border border-border-primary bg-surface-primary shadow-xl"
		>
			{#if title}
				<div
					class="flex items-center justify-between gap-3 border-b border-border-secondary px-4 py-3"
				>
					<h2 id={titleId} class="min-w-0 truncate text-lg font-medium text-text-primary" {title}>
						{title}
					</h2>
					<div class="flex shrink-0 items-center gap-2">
						{@render headerActions?.()}
						<button
							type="button"
							class="rounded p-1 text-text-secondary transition-colors hover:text-text-primary"
							onclick={onclose}
							disabled={busy}
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
	</dialog>
{/if}
