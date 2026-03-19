<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		variant?: 'default' | 'success' | 'warning' | 'danger' | 'info';
		children: Snippet;
	}

	let { variant = 'default', children }: Props = $props();

	const variantClasses: Record<string, string> = {
		default: 'bg-surface-elevated text-text-secondary',
		success: 'bg-success/20 text-success',
		warning: 'bg-warning/20 text-warning',
		danger: 'bg-danger/20 text-danger',
		info: 'bg-accent/20 text-accent'
	};
</script>

<span
	class="inline-flex items-center rounded px-2 py-0.5 text-xs font-medium {variantClasses[variant]}"
>
	{@render children()}
</span>
