<script lang="ts">
	import type { IconProps } from 'lucide-svelte';
	import type { Snippet } from 'svelte';

	let { name, props }: { name: string; props: IconProps & { children?: Snippet } } = $props();
	const {
		size = 24,
		color = 'currentColor',
		strokeWidth = 2,
		absoluteStrokeWidth,
		class: className,
		children,
		...rest
	} = $derived(props);
	const hasLabel = $derived(
		Object.keys(rest).some((key) => key === 'role' || key.startsWith('aria-'))
	);
</script>

<svg
	xmlns="http://www.w3.org/2000/svg"
	viewBox="0 0 24 24"
	fill="none"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden={hasLabel ? undefined : 'true'}
	{...rest}
	width={size}
	height={size}
	stroke={color}
	stroke-width={absoluteStrokeWidth ? (Number(strokeWidth) * 24) / Number(size) : strokeWidth}
	class={['lucide-icon', 'lucide', 'lucide-' + name, className]}
>
	<use href={'/boxbox-icons.svg#' + name}></use>
	{@render children?.()}
</svg>
