<script lang="ts">
	/**
	 * Toast notification container component
	 * Displays toast notifications from the toastStore
	 */
	import { toastStore } from '$lib/stores/toast.svelte';
	import { CheckCircle, XCircle, Info, AlertTriangle, X } from 'lucide-svelte';
	import { fly, fade } from 'svelte/transition';

	const iconMap = {
		success: CheckCircle,
		error: XCircle,
		info: Info,
		warning: AlertTriangle
	};

	const colorMap = {
		success: 'bg-success/10 border-success/30 text-success',
		error: 'bg-danger/10 border-danger/30 text-danger',
		info: 'bg-accent/10 border-accent/30 text-accent',
		warning: 'bg-warning/10 border-warning/30 text-warning'
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
</script>

{#if toastStore.toasts.length > 0}
	<div class="pointer-events-none fixed right-4 bottom-4 z-[100] flex max-w-sm flex-col gap-2">
		{#each toastStore.toasts as toast (toast.id)}
			{@const ToastIcon = iconMap[toast.type]}
			<div
				class="pointer-events-auto flex items-start gap-3 rounded-lg border px-4 py-3 shadow-lg backdrop-blur-sm {colorMap[
					toast.type
				]}"
				in:fly={{ x: 100, duration: 200 }}
				out:fade={{ duration: 150 }}
				role="alert"
			>
				<!-- Icon -->
				<div class="mt-0.5 shrink-0 {iconColorMap[toast.type]}">
					<ToastIcon size={18} />
				</div>

				<!-- Message -->
				<p class="m-0 flex-1 pr-2 text-sm text-text-primary">
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
	</div>
{/if}
