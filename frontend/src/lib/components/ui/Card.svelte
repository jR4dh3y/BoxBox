<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		variant?: 'default' | 'interactive';
		padding?: 'none' | 'sm' | 'md' | 'lg';
		children: Snippet;
		onclick?: () => void;
	}

	let { variant = 'default', padding = 'md', children, onclick }: Props = $props();

	const baseClasses = 'bg-surface-secondary border border-border-primary rounded-md';

	const variantClasses: Record<string, string> = {
		default: '',
		interactive: 'cursor-pointer transition-all duration-150 hover:bg-surface-tertiary hover:border-border-focus',
	};

	const paddingClasses: Record<string, string> = {
		none: '',
		sm: 'p-2',
		md: 'p-4',
		lg: 'p-6',
	};

	const isButton = $derived(variant === 'interactive' && onclick);
</script>

{#if isButton}
	<button
		type="button"
		class="{baseClasses} {variantClasses[variant]} {paddingClasses[padding]} w-full text-left"
		{onclick}
	>
		{@render children()}
	</button>
{:else}
	<div class="{baseClasses} {variantClasses[variant]} {paddingClasses[padding]}">
		{@render children()}
	</div>
{/if}
