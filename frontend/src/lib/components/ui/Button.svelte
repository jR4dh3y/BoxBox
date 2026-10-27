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
		primary: 'bg-accent-solid text-white hover:enabled:bg-accent-solid-hover',
		secondary:
			'bg-surface-secondary border border-border-primary text-text-secondary hover:enabled:bg-surface-tertiary hover:enabled:text-text-primary',
		ghost:
			'bg-transparent text-text-secondary hover:enabled:bg-surface-secondary hover:enabled:text-text-primary',
		danger: 'bg-danger text-white hover:enabled:bg-danger-hover'
	};

	const sizeClasses: Record<string, string> = {
		sm: 'h-7 px-2.5 text-xs gap-1',
		md: 'h-8 px-3 text-[13px] gap-2',
		lg: 'h-9 px-5 text-sm gap-2',
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
