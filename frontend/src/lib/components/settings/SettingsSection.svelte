<script lang="ts">
	/**
	 * Collapsible settings section component
	 * Provides consistent header styling and expand/collapse behavior
	 */
	import { ChevronDown } from 'lucide-svelte';
	import type { Snippet, SvelteComponent } from 'svelte';

	interface Props {
		title: string;
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		icon: any;
		collapsed?: boolean;
		children?: Snippet;
	}

	let { title, icon: Icon, collapsed = $bindable(false), children }: Props = $props();

	const sectionHeaderClass =
		'w-full flex items-center gap-1.5 px-3 py-2.5 bg-transparent border-none text-text-secondary text-[11px] font-medium uppercase tracking-wide cursor-pointer text-left hover:text-text-primary';
</script>

<div class="border-b border-border-secondary">
	<button
		type="button"
		class={sectionHeaderClass}
		onclick={() => (collapsed = !collapsed)}
	>
		<ChevronDown
			size={14}
			class="shrink-0 transition-transform duration-150 {collapsed ? '-rotate-90' : ''}"
		/>
		<Icon size={14} class="shrink-0 opacity-60" />
		<span>{title}</span>
	</button>
	{#if !collapsed}
		<div class="pb-3">
			{@render children?.()}
		</div>
	{/if}
</div>
