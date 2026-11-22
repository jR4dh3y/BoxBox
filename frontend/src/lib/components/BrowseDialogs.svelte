<script lang="ts">
	import type { FileInfo } from '#lib/api/files.js';
	import { Button, Input, Modal } from '#lib/components/ui/index.js';

	import { isNativePath } from '#lib/api/desktop.js';
	import type { CreateDialog } from '#lib/browse/actions.svelte.js';
	type DeleteDialog = { open: boolean; items: FileInfo[] };

	let {
		createDialog,
		deleteDialog,
		onCreateNameChange,
		onCreateConfirm,
		onDeleteConfirm,
		onCloseCreate,
		onCloseDelete
	}: {
		createDialog: CreateDialog;
		deleteDialog: DeleteDialog;
		onCreateNameChange: (name: string) => void;
		onCreateConfirm: () => void;
		onDeleteConfirm: () => void;
		onCloseCreate: () => void;
		onCloseDelete: () => void;
	} = $props();

	function inputValue(event: Event): string {
		return (event.target as HTMLInputElement).value;
	}

	const deleteToTrash = $derived(
		deleteDialog.items.length > 0 && deleteDialog.items.every((item) => isNativePath(item.path))
	);
	const deleteCount = $derived(deleteDialog.items.length);
	const deleteFirst = $derived(deleteDialog.items[0]);
</script>

<Modal
	open={createDialog.open}
	title={createDialog.type === 'file' ? 'New file' : 'New folder'}
	onclose={onCloseCreate}
>
	<Input
		value={createDialog.name}
		selectOnFocus={createDialog.type === 'file' ? 'basename' : 'all'}
		oninput={(event) => onCreateNameChange(inputValue(event))}
		placeholder={createDialog.type === 'file' ? 'untitled.txt' : 'New folder'}
		onkeydown={(event) => event.key === 'Enter' && onCreateConfirm()}
	/>
	{#if createDialog.problem}
		<p class="m-0 mt-2 text-[13px] text-danger" role="alert">{createDialog.problem}</p>
	{/if}
	{#snippet footer()}
		<Button variant="primary" disabled={createDialog.problem !== null} onclick={onCreateConfirm}
			>Create</Button
		>
	{/snippet}
</Modal>

<Modal
	open={deleteDialog.open}
	title={deleteToTrash ? 'Move to Trash' : 'Delete'}
	onclose={onCloseDelete}
>
	<div class="flex flex-col gap-3 text-[13px] text-text-secondary">
		{#if deleteCount === 1 && deleteFirst}
			<p class="m-0 break-all">
				<span class="text-text-primary">{deleteFirst.name}</span>
				{deleteToTrash ? 'will be moved to Trash.' : 'will be deleted permanently.'}
			</p>
		{:else}
			<p class="m-0">
				{deleteCount} items {deleteToTrash
					? 'will be moved to Trash.'
					: 'will be deleted permanently.'}
			</p>
			<ul class="m-0 max-h-40 list-none overflow-auto p-0">
				{#each deleteDialog.items as item (item.path)}
					<li class="truncate py-0.5 text-text-primary" title={item.path}>{item.name}</li>
				{/each}
			</ul>
		{/if}
	</div>
	{#snippet footer()}
		<Button variant="danger" onclick={onDeleteConfirm}
			>{deleteToTrash ? 'Move to Trash' : 'Delete'}</Button
		>
	{/snippet}
</Modal>
