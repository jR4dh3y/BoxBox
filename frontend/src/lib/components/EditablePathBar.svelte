<script lang="ts">
	import type { MountPoint } from '#lib/api/files.js';
	import { Home } from 'lucide-svelte';
	import { tick } from 'svelte';
	import LoadError from '#lib/components/LoadError.svelte';
	import { rootLabel } from '#lib/browse/rootLabel.js';

	let {
		pathSegments = [],
		roots = [],
		pageTitle = null,
		onNavigate,
		includeHiddenSuggestions = false
	}: {
		pathSegments?: string[];
		roots?: MountPoint[];
		/** Shown instead of the path while the pane shows a page rather than a folder. */
		pageTitle?: string | null;
		onNavigate?: (path: string) => void;
		includeHiddenSuggestions?: boolean;
	} = $props();

	const breadcrumbs = $derived.by(() => {
		const path = pathSegments.join('/');
		const root = roots.find((root) => path === root.name || path.startsWith(`${root.name}/`));
		if (root) {
			const label = root.serverName
				? `${root.serverName} · ${root.label ?? root.name}`
				: (root.label ?? root.name);
			const suffix = path.slice(root.name.length).split('/').filter(Boolean);
			return [
				{ label, path: root.name },
				...suffix.map((label, index) => ({
					label,
					path: `${root.name}/${suffix.slice(0, index + 1).join('/')}`
				}))
			];
		}
		return pathSegments[0] === '@remote'
			? pathSegments
					.slice(2)
					.map((label, index) => ({ label, path: pathSegments.slice(0, index + 3).join('/') }))
			: pathSegments.map((label, index) => ({
					label,
					path: pathSegments.slice(0, index + 1).join('/')
				}));
	});
	let editing = $state(false);
	let pathBar = $state<HTMLDivElement>();

	/** Open the location editor; it focuses and selects the path itself. */
	export function edit(): void {
		editing = true;
	}

	function closeEditor(): void {
		editing = false;
		// Keep focus the user moved elsewhere; reclaim it only when the editor took it with it.
		void tick().then(() => {
			if (!document.activeElement || document.activeElement === document.body) pathBar?.focus();
		});
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
		<PathEditor
			{pathSegments}
			{roots}
			{onNavigate}
			{includeHiddenSuggestions}
			onClose={closeEditor}
		/>
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
		{#if pageTitle}
			<span class="text-[13px] whitespace-nowrap text-text-primary">{pageTitle}</span>
		{:else if pathSegments.length === 0}
			<span class="text-[13px] whitespace-nowrap text-text-primary">{rootLabel()}</span>
		{:else}
			<div class="flex flex-1 items-center gap-1 overflow-hidden">
				{#each breadcrumbs as segment, index (segment.path)}
					{#if index > 0}<span class="text-xs text-text-muted">/</span>{/if}
					{#if index === breadcrumbs.length - 1}
						<span class="text-[13px] whitespace-nowrap text-text-primary">{segment.label}</span>
					{:else}
						<button
							type="button"
							class="cursor-pointer border-none bg-transparent p-0 text-[13px] text-text-secondary hover:text-white hover:underline"
							onclick={(event) => {
								event.stopPropagation();
								onNavigate?.(segment.path);
							}}>{segment.label}</button
						>
					{/if}
				{/each}
			</div>
		{/if}
	</div>
{/if}
