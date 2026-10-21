<script lang="ts">
	import { onMount } from 'svelte';
	import { File, Folder, RefreshCw, RotateCcw, Trash2 } from 'lucide-svelte';
	import { Button, Modal, Spinner } from '#lib/components/ui/index.js';
	import { DesktopTrashController } from '#lib/api/desktopTrashController.svelte.js';
	import type { DesktopTrashItem } from '#lib/api/desktopTrash.js';

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

	function formatDate(value: string): string {
		return new Intl.DateTimeFormat(undefined, {
			dateStyle: 'medium',
			timeStyle: 'short'
		}).format(new Date(value));
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
				<p class="m-0 text-sm text-text-secondary">{controller.loadState.message}</p>
				<Button variant="secondary" size="sm" onclick={() => controller.load()}>Try again</Button>
			</div>
		{:else if items.length === 0}
			<div class="flex flex-1 flex-col items-center justify-center gap-2 py-10 text-center">
				<Trash2 size={24} class="text-text-muted" />
				<p class="m-0 text-sm font-medium text-text-primary">Trash is empty</p>
				<p class="m-0 max-w-sm text-sm text-text-secondary">
					Items moved to Trash will stay here until you restore or delete them permanently.
				</p>
			</div>
		{:else}
			{#if controller.refreshError}
				<p
					class="m-0 rounded border border-border-primary bg-surface-secondary px-3 py-2 text-sm text-text-secondary"
					role="status"
				>
					Refresh failed: {controller.refreshError}
				</p>
			{/if}
			{#if controller.actionState.kind === 'failed'}
				<p
					class="m-0 rounded border border-border-primary bg-surface-secondary px-3 py-2 text-sm text-text-secondary"
					role="alert"
				>
					{controller.actionState.message}
				</p>
			{/if}
			<ul
				class="m-0 max-h-[55vh] list-none overflow-y-auto rounded border border-border-primary p-0"
				aria-label="Trash items"
			>
				{#each items as item (item.id)}
					<li class="border-b border-border-secondary last:border-b-0">
						<button
							type="button"
							class="flex w-full items-start gap-3 px-3 py-3 text-left transition-colors hover:bg-surface-secondary focus-visible:outline-2 focus-visible:outline-accent {controller.selectedId ===
							item.id
								? 'bg-surface-secondary'
								: ''}"
							aria-pressed={controller.selectedId === item.id}
							disabled={busy}
							onclick={() => select(item)}
						>
							<span class="mt-0.5 shrink-0 text-text-secondary">
								{#if item.isDir}<Folder size={18} />{:else}<File size={18} />{/if}
							</span>
							<span class="min-w-0 flex-1">
								<span class="block truncate text-sm font-medium text-text-primary" title={item.name}
									>{item.name}</span
								>
								<span
									class="mt-1 block truncate text-xs text-text-secondary"
									title={item.originalPath || 'Original location unavailable'}
								>
									{item.originalPath || 'Original location unavailable'}
								</span>
								<span class="mt-1 block text-xs text-text-muted"
									>Deleted {formatDate(item.deletedAt)}</span
								>
							</span>
							{#if item.canRestore}
								<RotateCcw size={15} class="mt-1 shrink-0 text-text-muted" aria-hidden="true" />
							{/if}
						</button>
					</li>
				{/each}
			</ul>
		{/if}

		{#if confirmingDelete && selectedItem}
			<p
				class="m-0 rounded border border-border-primary bg-surface-secondary px-3 py-2 text-sm text-text-primary"
				role="alert"
			>
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
				<Trash2 size={15} />
				{busy ? 'Deleting…' : 'Delete permanently'}
			</Button>
		{:else}
			<Button
				variant="secondary"
				disabled={!selectedItem || busy}
				onclick={() => (confirmingDelete = true)}
			>
				<Trash2 size={15} />Delete permanently
			</Button>
			<Button
				variant="primary"
				disabled={!selectedItem?.canRestore || busy}
				onclick={restoreSelected}
			>
				<RotateCcw size={15} />
				{busy ? 'Working…' : 'Restore'}
			</Button>
		{/if}
	{/snippet}
</Modal>
