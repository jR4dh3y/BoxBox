<script lang="ts">
	interface Props {
		/** Omit the value when the amount of work is unknown. */
		value?: number;
		max?: number;
		variant?: 'default' | 'info' | 'success' | 'warning' | 'danger';
		size?: 'sm' | 'md';
		showLabel?: boolean;
		label?: string;
	}

	let {
		value,
		max = 100,
		variant = 'default',
		size = 'md',
		showLabel = false,
		label
	}: Props = $props();

	const percentage = $derived(
		value === undefined ? undefined : Math.min(100, Math.max(0, (value / max) * 100))
	);

	const variantClasses: Record<string, string> = {
		default: 'bg-accent',
		info: 'bg-info',
		success: 'bg-success',
		warning: 'bg-warning',
		danger: 'bg-danger'
	};

	const sizeClasses: Record<string, string> = {
		sm: 'h-1',
		md: 'h-2'
	};
</script>

<div class="w-full">
	<div
		class="relative w-full overflow-hidden rounded-full bg-surface-elevated {sizeClasses[size]}"
		role="progressbar"
		aria-label={label}
		aria-valuenow={value}
		aria-valuemin={0}
		aria-valuemax={max}
	>
		{#if percentage === undefined}
			<div
				class="{variantClasses[
					variant
				]} absolute inset-y-0 w-1/3 rounded-full opacity-70 motion-safe:animate-progress-sweep motion-reduce:w-full motion-reduce:opacity-40"
			></div>
		{:else}
			<div
				class="{variantClasses[variant]} h-full rounded-full transition-[width] duration-300"
				style="width: {percentage}%"
			></div>
		{/if}
	</div>
	{#if showLabel && percentage !== undefined}
		<span class="mt-1 text-xs text-text-secondary">{percentage.toFixed(0)}%</span>
	{/if}
</div>
