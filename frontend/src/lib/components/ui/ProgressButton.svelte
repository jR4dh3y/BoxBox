<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
		size?: 'sm' | 'md' | 'lg' | 'icon';
		disabled?: boolean;
		type?: 'button' | 'submit' | 'reset';
		title?: string;
		children: Snippet;
		onclick?: (e: MouseEvent) => void;
		progress?: number | null;
		max?: number;
		progressVariant?: 'default' | 'success' | 'warning' | 'danger';
		progressLabel?: string;
		busy?: boolean;
		className?: string;
	}

	let {
		variant = 'primary',
		size = 'md',
		disabled = false,
		type = 'button',
		title,
		children,
		onclick,
		progress = null,
		max = 100,
		progressVariant = 'default',
		progressLabel = '',
		busy = false,
		className = ''
	}: Props = $props();

	const percentage = $derived(
		progress === null || max <= 0 ? 0 : Math.min(100, Math.max(0, (progress / max) * 100))
	);
	const showProgress = $derived(progress !== null && progress > 0);
	const isDisabled = $derived(disabled || busy);
	const accessibleTitle = $derived(progressLabel || title);

	const baseClasses =
		'relative isolate inline-flex cursor-pointer items-center justify-center overflow-hidden rounded font-medium transition-all duration-150 disabled:cursor-not-allowed';

	const variantClasses: Record<string, string> = {
		primary: 'bg-accent text-white hover:enabled:bg-accent-hover',
		secondary:
			'border border-border-primary bg-surface-secondary text-text-secondary hover:enabled:bg-surface-tertiary hover:enabled:text-text-primary',
		ghost:
			'bg-transparent text-text-secondary hover:enabled:bg-surface-secondary hover:enabled:text-text-primary',
		danger: 'bg-danger text-white hover:enabled:bg-danger-hover'
	};

	const progressShellClasses =
		'border border-border-focus bg-surface-secondary text-text-primary shadow-[inset_0_0_0_1px_rgba(255,255,255,0.03)]';

	const sizeClasses: Record<string, string> = {
		sm: 'px-2 py-1 text-xs gap-1',
		md: 'px-4 py-2 text-sm gap-2',
		lg: 'px-6 py-3 text-base gap-2',
		icon: 'h-7 w-7 p-0'
	};

	const contentGapClasses: Record<string, string> = {
		sm: 'gap-1',
		md: 'gap-2',
		lg: 'gap-2',
		icon: 'gap-0'
	};

	const fillClasses: Record<string, string> = {
		default: 'bg-accent/70',
		success: 'bg-success/75',
		warning: 'bg-warning/75',
		danger: 'bg-danger/80'
	};
</script>

<button
	{type}
	class="{baseClasses} {showProgress ? progressShellClasses : variantClasses[variant]} {sizeClasses[
		size
	]} {isDisabled && !showProgress ? 'opacity-50' : ''} {className}"
	disabled={isDisabled}
	title={accessibleTitle}
	aria-busy={busy || undefined}
	{onclick}
>
	{#if showProgress}
		<span
			class="absolute inset-y-0 left-0 -z-10 rounded-l {fillClasses[
				progressVariant
			]} transition-all duration-300"
			style:width={percentage + '%'}
		></span>
	{/if}
	<span
		class="relative inline-flex items-center justify-center {contentGapClasses[
			size
		]} whitespace-nowrap"
	>
		{@render children()}
	</span>
	{#if progressLabel}
		<span class="sr-only" role="status" aria-live="polite">{progressLabel}</span>
	{/if}
</button>
