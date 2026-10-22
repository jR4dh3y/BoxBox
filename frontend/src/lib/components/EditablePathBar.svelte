<script lang="ts">
	import { Home } from 'lucide-svelte';
	import { tick } from 'svelte';
	import LoadError from '#lib/components/LoadError.svelte';

	let {
		pathSegments = [],
		onNavigate,
		includeHiddenSuggestions = false
	}: {
		pathSegments?: string[];
		onNavigate?: (path: string) => void;
		includeHiddenSuggestions?: boolean;
	} = $props();

	let editing = $state(false);
	let pathBar = $state<HTMLDivElement>();

	function closeEditor(): void {
		editing = false;
		void tick().then(() => pathBar?.focus());
	}

	function activate(event: MouseEvent | KeyboardEvent): void {
		if (event.target instanceof Element && event.target.closest('button')) return;
		if (event instanceof KeyboardEvent) {
			if (event.key !== 'Enter' && event.key !== 'F2') return;
			event.preventDefault();
		}
		editing = true;
	}
</script>

<svelte:window
	onkeydown={(event) => {
		if (editing && event.key === 'Escape') {
			event.preventDefault();
			closeEditor();
		}
	}}
/>

{#if editing}
	{#await import('./PathEditor.svelte')}
		<div class="flex min-w-0 flex-1 items-center px-2 py-1 text-sm text-text-muted" role="status">
			Opening location editor…
		</div>
	{:then { default: PathEditor }}
		<PathEditor {pathSegments} {onNavigate} {includeHiddenSuggestions} onClose={closeEditor} />
	{:catch}
		<LoadError label="location editor" onClose={closeEditor} />
	{/await}
{:else}
	<div
		bind:this={pathBar}
		class="relative flex min-w-0 flex-1 cursor-text items-center gap-1.5 rounded border border-border-primary bg-surface-secondary px-2 py-1 focus-within:border-border-focus"
		role="button"
		tabindex="0"
		aria-label="Edit location"
		onclick={activate}
		onkeydown={activate}
	>
		<button
			type="button"
			class="flex h-4.5 w-4.5 shrink-0 cursor-pointer items-center justify-center border-none bg-transparent text-text-secondary hover:text-text-primary"
			onclick={(event) => {
				event.stopPropagation();
				onNavigate?.('');
			}}
			title="Go to root"
		>
			<Home size={14} />
		</button>
		{#if pathSegments.length === 0}
			<span class="text-[13px] whitespace-nowrap text-text-primary">This Server</span>
		{:else}
			<div class="flex flex-1 items-center gap-1 overflow-hidden">
				{#each pathSegments as segment, index (index)}
					{#if index > 0}<span class="text-xs text-text-muted">/</span>{/if}
					{#if index === pathSegments.length - 1}
						<span class="text-[13px] whitespace-nowrap text-text-primary">{segment}</span>
					{:else}
						<button
							type="button"
							class="cursor-pointer border-none bg-transparent p-0 text-[13px] text-text-secondary hover:text-white hover:underline"
							onclick={(event) => {
								event.stopPropagation();
								onNavigate?.(pathSegments.slice(0, index + 1).join('/'));
							}}>{segment}</button
						>
					{/if}
				{/each}
			</div>
		{/if}
	</div>
{/if}
