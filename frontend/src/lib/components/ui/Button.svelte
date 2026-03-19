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
	}

	let {
		variant = 'primary',
		size = 'md',
		disabled = false,
		type = 'button',
		title,
		children,
		onclick
	}: Props = $props();

	const baseClasses =
		'inline-flex items-center justify-center font-medium rounded transition-all duration-150 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed';

	const variantClasses: Record<string, string> = {
		primary: 'bg-accent text-white hover:enabled:bg-accent-hover',
		secondary:
			'bg-surface-secondary border border-border-primary text-text-secondary hover:enabled:bg-surface-tertiary hover:enabled:text-text-primary',
		ghost:
			'bg-transparent text-text-secondary hover:enabled:bg-surface-secondary hover:enabled:text-text-primary',
		danger: 'bg-danger text-white hover:enabled:bg-danger-hover'
	};

	const sizeClasses: Record<string, string> = {
		sm: 'px-2 py-1 text-xs gap-1',
		md: 'px-4 py-2 text-sm gap-2',
		lg: 'px-6 py-3 text-base gap-2',
		icon: 'w-7 h-7 p-0'
	};
</script>

<button
	{type}
	class="{baseClasses} {variantClasses[variant]} {sizeClasses[size]}"
	{disabled}
	{title}
	{onclick}
>
	{@render children()}
</button>
