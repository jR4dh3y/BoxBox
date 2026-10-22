<script lang="ts">
	import { onMount } from 'svelte';
	import { File, Folder, RefreshCw } from 'lucide-svelte';
	import { Button, Modal, Spinner } from '#lib/components/ui/index.js';
	import { CONFIG } from '#lib/config.js';
	import { fullFileDate, parentFolder } from '#lib/browse/details.js';
	import { TrashController, type TrashAction } from '#lib/desktop/trash.svelte.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';

	interface Props {
		onClose: () => void;
		onChanged?: () => void;
	}

	let { onClose, onChanged }: Props = $props();

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
		if (await controller.run(kind)) onChanged?.();
	}
</script>

<Modal open title="Trash" size="lg" busy={action.busy} onclose={onClose}>
	{#snippet headerActions()}
		<Button
			variant="ghost"
			size="icon"
			title="Refresh Trash"
			disabled={items.refreshing || action.busy}
			onclick={() => void items.load()}
		>
			<RefreshCw size={16} class={items.refreshing ? 'animate-spin' : ''} />
			<span class="sr-only">Refresh Trash</span>
		</Button>
	{/snippet}

	<div class="flex min-h-64 flex-col gap-3 text-[13px]">
		{#if items.state.kind === 'loading'}
			{#if showSpinner.value}
				<div class="flex items-center gap-2 py-10 text-text-secondary" role="status">
					<Spinner size="sm" />Loading Trash…
				</div>
			{/if}
		{:else if items.state.kind === 'failed'}
			<p class="m-0 flex items-center justify-between gap-3 text-danger" role="alert">
				{items.state.message}
				<Button variant="secondary" size="sm" onclick={() => void items.load()}>Try again</Button>
			</p>
		{:else if items.state.value.length === 0}
			<p class="m-0 py-10 text-center text-text-secondary">Trash is empty</p>
		{:else}
			<div class="max-h-[55vh] overflow-y-auto rounded border border-border-secondary">
				<table class="w-full table-fixed border-collapse" aria-label="Trash items">
					<thead>
						<tr class="text-left text-text-secondary">
							<th class="sticky top-0 h-8 w-[40%] bg-surface-secondary px-3 font-medium">Name</th>
							<th class="sticky top-0 h-8 bg-surface-secondary px-3 font-medium">From</th>
							<th class="sticky top-0 h-8 w-44 bg-surface-secondary px-3 font-medium">Deleted</th>
						</tr>
					</thead>
					<tbody>
						{#each items.state.value as item (item.id)}
							{@const from = parentFolder(item.originalPath)}
							{@const selected = controller.selectedId === item.id}
							<tr
								class="h-8 cursor-default border-t border-border-secondary {selected
									? 'bg-selection hover:bg-selection-hover'
									: 'hover:bg-surface-secondary'}"
								aria-selected={selected}
								tabindex={action.busy ? -1 : 0}
								onclick={() => controller.select(item.id)}
								onkeydown={(event) => event.key === 'Enter' && controller.select(item.id)}
							>
								<td class="truncate px-3" title={item.name}>
									<span class="inline-flex min-w-0 items-center gap-2 align-middle">
										{#if item.isDir}<Folder size={14} class="shrink-0 text-folder" />{:else}<File
												size={14}
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
			</div>
		{/if}

		{#if error}
			<p class="m-0 text-danger" role="alert">{error}</p>
		{:else if controller.confirmingDelete && controller.selected}
			<p class="m-0 text-danger" role="alert">
				“{controller.selected.name}” will be deleted permanently. This cannot be undone.
			</p>
		{/if}
	</div>

	{#snippet footer()}
		<Button
			variant={controller.confirmingDelete ? 'danger' : 'secondary'}
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
			disabled={!controller.selected?.canRestore || action.busy}
			onclick={() => run('restore')}
		>
			{action.state.kind === 'running' && action.state.action === 'restore'
				? 'Restoring…'
				: 'Restore'}
		</Button>
	{/snippet}
</Modal>
