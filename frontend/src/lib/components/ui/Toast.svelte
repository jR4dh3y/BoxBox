<script lang="ts">
	/**
	 * Toast notification container component
	 * Displays toast notifications from the toastStore
	 */
	import { toastStore } from '#lib/stores/toast.svelte.js';
	import { CheckCircle, XCircle, Info, AlertTriangle, X } from 'lucide-svelte';
	import { fly, fade } from 'svelte/transition';
	import { flip } from 'svelte/animate';

	const iconMap = {
		success: CheckCircle,
		error: XCircle,
		info: Info,
		warning: AlertTriangle
	};

	const iconColorMap = {
		success: 'text-success',
		error: 'text-danger',
		info: 'text-accent',
		warning: 'text-warning'
	};

	function handleDismiss(id: string) {
		toastStore.remove(id);
	}

	// Newer toasts wait their turn; a lifetime starts only once its toast is shown.
	const MAX_VISIBLE = 4;
	const visibleToasts = $derived(toastStore.toasts.slice(0, MAX_VISIBLE));
</script>

{#each visibleToasts as toast (toast.id)}
	{@const Icon = iconMap[toast.type]}
	<div
		class="pointer-events-auto flex items-start gap-3 rounded-lg border border-border-primary bg-surface-secondary px-4 py-3 shadow-lg"
		in:fly={{ x: 100, duration: 200 }}
		animate:flip={{ duration: 200 }}
		out:fade={{ duration: 150 }}
		{@attach () => toastStore.startLifetime(toast.id)}
		role="alert"
	>
		<!-- Icon -->
		<div class="mt-0.5 shrink-0 {iconColorMap[toast.type]}">
			<Icon size={16} />
		</div>

		<!-- Message -->
		<p class="m-0 flex-1 pr-2 text-[13px] text-text-primary">
			{toast.message}
		</p>

		<!-- Dismiss button -->
		<button
			type="button"
			class="shrink-0 rounded p-0.5 text-text-muted transition-colors hover:bg-surface-elevated hover:text-text-primary"
			onclick={() => handleDismiss(toast.id)}
			aria-label="Dismiss notification"
		>
			<X size={14} />
		</button>
	</div>
{/each}
