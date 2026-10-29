<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import { File, Folder, Trash2, Undo2 } from 'lucide-svelte';
	import { Button, Spinner } from '#lib/components/ui/index.js';
	import { CONFIG } from '#lib/config.js';
	import { fullFileDate, parentFolder } from '#lib/browse/details.js';
	import { TrashController, type TrashAction } from '#lib/desktop/trash.svelte.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';

	let {
		onChanged,
		selectionCheckboxes = true,
		toolbar
	}: {
		onChanged: () => void;
		selectionCheckboxes?: boolean;
		toolbar: Snippet<[Snippet, () => void, boolean, string]>;
	} = $props();

	const controller = new TrashController();
	const { items, action } = controller;
	const showSpinner = createDelayedFlag(
		() => items.state.kind === 'loading',
		CONFIG.ui.spinnerDelayMs
	);
	const error = $derived(action.error ?? items.refreshError);
	const deleteLabel = $derived(
		action.state.kind === 'running' && action.state.action === 'delete'
			? 'Deleting…'
			: controller.confirmingDelete
				? 'Confirm delete'
				: 'Delete permanently'
	);
	const restoreLabel = $derived(
		action.state.kind === 'running' && action.state.action === 'restore' ? 'Restoring…' : 'Restore'
	);

	onMount(() => {
		const request = new AbortController();
		void items.load(request.signal);
		return () => request.abort();
	});

	async function run(kind: TrashAction): Promise<void> {
		if (await controller.run(kind)) onChanged();
	}

	function rowKeydown(event: KeyboardEvent, id: string): void {
		if (event.target instanceof HTMLInputElement) return;
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			controller.select(id);
		} else if (event.key === 'Delete' && controller.selectedId === id) {
			event.preventDefault();
			void run('delete');
		}
	}
</script>

{#snippet trashActions()}
	<Button
		variant={controller.confirmingDelete ? 'danger' : 'secondary'}
		size="sm"
		disabled={!controller.selected || action.busy}
		onclick={() => run('delete')}
		title={deleteLabel}
	>
		<Trash2 size={14} />
		<span class="sr-only @xl:not-sr-only">{deleteLabel}</span>
	</Button>
	<Button
		variant="primary"
		size="sm"
		disabled={!controller.selected?.canRestore || action.busy}
		onclick={() => run('restore')}
		title={restoreLabel}
	>
		<Undo2 size={14} />
		<span class="sr-only @xl:not-sr-only">{restoreLabel}</span>
	</Button>
{/snippet}

<div class="flex h-full min-h-0 flex-col text-[13px]">
	{@render toolbar(trashActions, () => void items.load(), items.refreshing || action.busy, 'Trash')}
	{#if error || (controller.confirmingDelete && controller.selected)}
		<p class="m-0 shrink-0 border-b border-border-secondary px-3 py-2" role="alert">
			{#if error}
				<span class="text-danger">{error}</span>
			{:else if controller.confirmingDelete && controller.selected}
				<span class="text-danger"
					>“{controller.selected.name}” will be deleted permanently. This cannot be undone.</span
				>
			{/if}
		</p>
	{/if}

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
									<span
										class="group/icon relative flex h-5 w-5 shrink-0 items-center justify-center"
									>
										<span
											class="flex items-center justify-center {selectionCheckboxes
												? selected
													? 'invisible'
													: 'group-hover/icon:invisible pointer-coarse:invisible'
												: ''}"
										>
											{#if item.isDir}<Folder size={16} class="text-folder" />{:else}<File
													size={16}
													class="text-text-secondary"
												/>{/if}
										</span>
										{#if selectionCheckboxes}
											<input
												type="checkbox"
												checked={selected}
												disabled={action.busy}
												aria-label={`Select ${item.name}`}
												class="absolute inset-0.5 m-0 cursor-pointer accent-accent {selected
													? ''
													: 'opacity-0 group-hover/icon:opacity-100 focus-visible:opacity-100 pointer-coarse:opacity-100'}"
												onclick={(event) => event.stopPropagation()}
												ondblclick={(event) => event.stopPropagation()}
												onchange={(event) =>
													controller.select(event.currentTarget.checked ? item.id : null)}
											/>
										{/if}
									</span>
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
