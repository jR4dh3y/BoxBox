<script lang="ts">
	import type { Snippet } from 'svelte';
	import { tick } from 'svelte';
	import { X } from 'lucide-svelte';

	interface Props {
		open?: boolean;
		title?: string;
		ariaLabel?: string;
		children: Snippet;
		footer?: Snippet;
		onclose?: () => void;
	}

	let { open = false, title, ariaLabel, children, footer, onclose }: Props = $props();

	const titleId = `modal-title-${Math.random().toString(36).slice(2, 10)}`;
	const resolvedAriaLabel = $derived(title ? undefined : (ariaLabel ?? 'Dialog'));

	let dialogRef: HTMLDivElement | null = $state(null);
	let previousActiveElement: HTMLElement | null = $state(null);
	let wasOpen = false;

	function getFocusableElements(): HTMLElement[] {
		if (!dialogRef) {
			return [];
		}

		const selector =
			'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';
		const elements = Array.from(dialogRef.querySelectorAll<HTMLElement>(selector));
		return elements.filter((element) => !element.hasAttribute('inert'));
	}

	async function focusInitialElement(): Promise<void> {
		await tick();
		if (!open) {
			return;
		}

		const focusableElements = getFocusableElements();
		const autofocusElement = focusableElements.find((element) => element.hasAttribute('autofocus'));
		const firstElement = autofocusElement ?? focusableElements[0] ?? dialogRef;
		firstElement?.focus();
	}

	function restoreFocus(): void {
		if (previousActiveElement && typeof previousActiveElement.focus === 'function') {
			previousActiveElement.focus();
		}
		previousActiveElement = null;
	}

	$effect(() => {
		if (open && !wasOpen) {
			wasOpen = true;
			previousActiveElement =
				document.activeElement instanceof HTMLElement ? document.activeElement : null;
			void focusInitialElement();
		}

		if (!open && wasOpen) {
			wasOpen = false;
			restoreFocus();
		}
	});

	$effect(() => {
		return () => {
			restoreFocus();
		};
	});

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onclose?.();
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onclose?.();
			return;
		}

		if (e.key !== 'Tab') {
			return;
		}

		const focusableElements = getFocusableElements();
		if (focusableElements.length === 0) {
			e.preventDefault();
			dialogRef?.focus();
			return;
		}

		const firstElement = focusableElements[0];
		const lastElement = focusableElements[focusableElements.length - 1];
		const activeElement = document.activeElement as HTMLElement | null;

		if (e.shiftKey && activeElement === firstElement) {
			e.preventDefault();
			lastElement.focus();
			return;
		}

		if (!e.shiftKey && activeElement === lastElement) {
			e.preventDefault();
			firstElement.focus();
		}
	}
</script>

{#if open}
	<div
		class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
		role="presentation"
		onclick={handleBackdropClick}
	>
		<div
			bind:this={dialogRef}
			class="mx-4 flex max-h-[90vh] w-full max-w-md flex-col overflow-hidden rounded-lg border border-border-primary bg-surface-primary shadow-xl"
			role="dialog"
			aria-modal="true"
			aria-labelledby={title ? titleId : undefined}
			aria-label={resolvedAriaLabel}
			tabindex="-1"
			onkeydown={handleKeydown}
		>
			{#if title}
				<div class="flex items-center justify-between border-b border-border-secondary px-4 py-3">
					<h2 id={titleId} class="text-lg font-medium text-text-primary">{title}</h2>
					<button
						type="button"
						class="rounded p-1 text-text-secondary transition-colors hover:text-text-primary"
						onclick={onclose}
						aria-label="Close"
					>
						<X size={18} />
					</button>
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
