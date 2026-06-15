<script lang="ts">
	interface Props {
		size?: 'sm' | 'md' | 'lg';
	}

	let { size = 'md' }: Props = $props();

	const sizeClasses: Record<string, string> = {
		sm: 'w-4 h-4 border',
		md: 'w-6 h-6 border-2',
		lg: 'w-8 h-8 border-2'
	};
</script>

<div
	class="{sizeClasses[size]} animate-spin rounded-full border-surface-elevated border-t-accent"
	role="status"
	aria-label="Loading"
></div>
