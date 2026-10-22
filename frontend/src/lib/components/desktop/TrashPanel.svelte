<script lang="ts">
	import { onMount } from 'svelte';
	import { File, Folder, RefreshCw } from 'lucide-svelte';
	import { Button, Modal, Spinner } from '#lib/components/ui/index.js';
	import { DesktopTrashController } from '#lib/api/desktopTrashController.svelte.js';
	import type { DesktopTrashItem } from '#lib/api/desktopTrash.js';
	import { fullFileDate, parentFolder } from '#lib/browse/details.js';

	interface Props {
		onClose: () => void;
		onChanged?: () => void;
	}

	let { onClose, onChanged }: Props = $props();
	const controller = new DesktopTrashController();
	let confirmingDelete = $state(false);

	const items = $derived(controller.loadState.kind === 'ready' ? controller.loadState.items : []);
	const selectedItem = $derived(items.find((item) => item.id === controller.selectedId) ?? null);
	const busy = $derived(controller.actionState.kind === 'running');

	onMount(() => {
		const request = new AbortController();
		void controller.load(request.signal);
		return () => request.abort();
	});

	function close(): void {
		if (!busy) onClose();
	}

	function select(item: DesktopTrashItem): void {
		controller.selectedId = item.id;
		confirmingDelete = false;
		controller.actionState = { kind: 'idle' };
	}

	async function restoreSelected(): Promise<void> {
		if (!selectedItem?.canRestore || busy) return;
		const restored = await controller.restore(selectedItem.id);
		if (restored) {
			confirmingDelete = false;
			onChanged?.();
		}
	}

	async function deleteSelected(): Promise<void> {
		if (!selectedItem || busy) return;
		const deleted = await controller.delete(selectedItem.id);
		if (deleted) {
			confirmingDelete = false;
			onChanged?.();
		}
	}
</script>

<Modal open title="Trash" size="lg" onclose={close}>
	{#snippet headerActions()}
		<Button
			variant="ghost"
			size="icon"
			title="Refresh Trash"
			disabled={controller.refreshing || busy}
			onclick={() => controller.load()}
		>
			<RefreshCw size={16} class={controller.refreshing ? 'animate-spin' : ''} />
			<span class="sr-only">Refresh Trash</span>
		</Button>
	{/snippet}

	<div class="flex min-h-64 flex-col gap-3">
		{#if controller.loadState.kind === 'loading'}
			<div class="flex flex-1 items-center justify-center gap-3 py-10 text-sm text-text-secondary">
				<Spinner size="sm" />
				<span>Loading Trash…</span>
			</div>
		{:else if controller.loadState.kind === 'failed'}
			<div class="flex flex-1 flex-col items-center justify-center gap-3 py-10 text-center">
				<p class="m-0 text-[13px] text-danger">{controller.loadState.message}</p>
				<Button variant="secondary" size="sm" onclick={() => controller.load()}>Try again</Button>
			</div>
		{:else if items.length === 0}
			<p class="m-0 py-10 text-center text-[13px] text-text-secondary">Trash is empty</p>
		{:else}
			{#if controller.refreshError}
				<p class="m-0 text-[13px] text-danger" role="status">
					Refresh failed: {controller.refreshError}
				</p>
			{/if}
			{#if controller.actionState.kind === 'failed'}
				<p class="m-0 text-[13px] text-danger" role="alert">{controller.actionState.message}</p>
			{/if}
			<div class="max-h-[55vh] overflow-y-auto rounded border border-border-secondary">
				<table class="w-full table-fixed border-collapse text-[13px]" aria-label="Trash items">
					<thead>
						<tr class="text-left text-text-secondary">
							<th class="sticky top-0 h-8 w-[40%] bg-surface-secondary px-3 font-medium">Name</th>
							<th class="sticky top-0 h-8 bg-surface-secondary px-3 font-medium">From</th>
							<th class="sticky top-0 h-8 w-44 bg-surface-secondary px-3 font-medium">Deleted</th>
						</tr>
					</thead>
					<tbody>
						{#each items as item (item.id)}
							{@const from = parentFolder(item.originalPath)}
							<tr
								class="h-8 cursor-default border-t border-border-secondary hover:bg-surface-secondary {controller.selectedId ===
								item.id
									? 'bg-selection hover:bg-selection-hover'
									: ''}"
								aria-selected={controller.selectedId === item.id}
								tabindex={busy ? -1 : 0}
								onclick={() => !busy && select(item)}
								onkeydown={(event) => event.key === 'Enter' && !busy && select(item)}
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

		{#if confirmingDelete && selectedItem}
			<p class="m-0 text-[13px] text-danger" role="alert">
				Permanently delete “{selectedItem.name}”? This cannot be undone.
			</p>
		{/if}
	</div>

	{#snippet footer()}
		{#if confirmingDelete && selectedItem}
			<Button variant="secondary" disabled={busy} onclick={() => (confirmingDelete = false)}
				>Cancel</Button
			>
			<Button variant="danger" disabled={busy} onclick={deleteSelected}>
				{busy ? 'Deleting…' : 'Delete permanently'}
			</Button>
		{:else}
			<Button
				variant="secondary"
				disabled={!selectedItem || busy}
				onclick={() => (confirmingDelete = true)}
				>Delete permanently
			</Button>
			<Button
				variant="primary"
				disabled={!selectedItem?.canRestore || busy}
				onclick={restoreSelected}
			>
				{busy ? 'Working…' : 'Restore'}
			</Button>
		{/if}
	{/snippet}
</Modal>
