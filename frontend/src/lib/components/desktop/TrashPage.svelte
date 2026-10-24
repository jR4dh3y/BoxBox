<script lang="ts">
	import { onMount } from 'svelte';
	import { File, Folder, RefreshCw } from 'lucide-svelte';
	import { Button, Spinner } from '#lib/components/ui/index.js';
	import { CONFIG } from '#lib/config.js';
	import { fullFileDate, parentFolder } from '#lib/browse/details.js';
	import { TrashController, type TrashAction } from '#lib/desktop/trash.svelte.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';

	let { onChanged }: { onChanged: () => void } = $props();

	const controller = new TrashController();
	const { items, action } = controller;
	const showSpinner = createDelayedFlag(
		() => items.state.kind === 'loading',
		CONFIG.ui.spinnerDelayMs
	);
	const error = $derived(action.error ?? items.refreshError);

	onMount(() => {
		const request = new AbortController();
		void items.load(request.signal);
		return () => request.abort();
	});

	async function run(kind: TrashAction): Promise<void> {
		if (await controller.run(kind)) onChanged();
	}

	function rowKeydown(event: KeyboardEvent, id: string): void {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			controller.select(id);
		} else if (event.key === 'Delete' && controller.selectedId === id) {
			event.preventDefault();
			void run('delete');
		}
	}
</script>

<div class="flex h-full min-h-0 flex-col text-[13px]">
	<div class="flex h-10 shrink-0 items-center gap-2 border-b border-border-secondary px-3">
		<p
			class="m-0 min-w-0 flex-1 truncate"
			role={error || controller.confirmingDelete ? 'alert' : undefined}
		>
			{#if error}
				<span class="text-danger">{error}</span>
			{:else if controller.confirmingDelete && controller.selected}
				<span class="text-danger"
					>“{controller.selected.name}” will be deleted permanently. This cannot be undone.</span
				>
			{/if}
		</p>
		<Button
			variant={controller.confirmingDelete ? 'danger' : 'secondary'}
			size="sm"
			disabled={!controller.selected || action.busy}
			onclick={() => run('delete')}
		>
			{#if action.state.kind === 'running' && action.state.action === 'delete'}
				Deleting…
			{:else if controller.confirmingDelete}
				Confirm delete
			{:else}
				Delete permanently
			{/if}
		</Button>
		<Button
			variant="primary"
			size="sm"
			disabled={!controller.selected?.canRestore || action.busy}
			onclick={() => run('restore')}
		>
			{action.state.kind === 'running' && action.state.action === 'restore'
				? 'Restoring…'
				: 'Restore'}
		</Button>
		<Button
			variant="ghost"
			size="icon"
			title="Reload Trash"
			disabled={items.refreshing || action.busy}
			onclick={() => void items.load()}
		>
			<RefreshCw size={16} class={items.refreshing ? 'animate-spin' : ''} />
			<span class="sr-only">Reload Trash</span>
		</Button>
	</div>

	<div class="min-h-0 flex-1 overflow-auto">
		{#if items.state.kind === 'loading'}
			{#if showSpinner.value}
				<div class="flex items-center justify-center gap-2 py-10 text-text-secondary" role="status">
					<Spinner size="sm" />Loading Trash…
				</div>
			{/if}
		{:else if items.state.kind === 'failed'}
			<p class="m-0 flex items-center justify-center gap-3 py-10 text-danger" role="alert">
				{items.state.message}
				<Button variant="secondary" size="sm" onclick={() => void items.load()}>Try again</Button>
			</p>
		{:else if items.state.value.length === 0}
			<p class="m-0 py-12 text-center text-text-muted">Trash is empty</p>
		{:else}
			<table class="w-full table-fixed border-collapse" aria-label="Trash items">
				<thead>
					<tr class="text-left text-text-secondary">
						<th
							class="sticky top-0 h-8 w-[40%] border-b border-border-primary bg-surface-secondary px-3 font-medium"
							>Name</th
						>
						<th
							class="sticky top-0 h-8 border-b border-border-primary bg-surface-secondary px-3 font-medium"
							>Original location</th
						>
						<th
							class="sticky top-0 h-8 w-44 border-b border-border-primary bg-surface-secondary px-3 font-medium"
							>Deleted</th
						>
					</tr>
				</thead>
				<tbody>
					{#each items.state.value as item (item.id)}
						{@const from = parentFolder(item.originalPath)}
						{@const selected = controller.selectedId === item.id}
						<tr
							class="h-8 cursor-default border-b border-border-secondary focus:outline-none {selected
								? 'bg-selection hover:bg-selection-hover'
								: 'hover:bg-surface-secondary focus:bg-surface-secondary'}"
							aria-selected={selected}
							tabindex={action.busy ? -1 : 0}
							onclick={() => controller.select(item.id)}
							onkeydown={(event) => rowKeydown(event, item.id)}
						>
							<td class="truncate px-3" title={item.name}>
								<span class="inline-flex min-w-0 items-center gap-2 align-middle">
									{#if item.isDir}<Folder size={16} class="shrink-0 text-folder" />{:else}<File
											size={16}
											class="shrink-0 text-text-secondary"
										/>{/if}
									<span class="truncate text-text-primary">{item.name}</span>
								</span>
							</td>
							<td class="truncate px-3 text-text-secondary" title={from}>{from || '—'}</td>
							<td class="truncate px-3 text-text-secondary">{fullFileDate(item.deletedAt)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>
