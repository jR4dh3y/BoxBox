<script lang="ts">
	import type { FileInfo } from '#lib/api/files.js';
	import { Button, Input, Modal } from '#lib/components/ui/index.js';

	type CreateDialog = { open: boolean; type: 'file' | 'directory'; name: string };
	import { isNativePath } from '#lib/api/desktop.js';
	import type { RenameDialog } from '#lib/browse/actions.svelte.js';
	type DeleteDialog = { open: boolean; items: FileInfo[] };

	let {
		createDialog,
		renameDialog,
		deleteDialog,
		onCreateNameChange,
		onRenameNameChange,
		onCreateConfirm,
		onRenameConfirm,
		onDeleteConfirm,
		onCloseCreate,
		onCloseRename,
		onCloseDelete
	}: {
		createDialog: CreateDialog;
		renameDialog: RenameDialog;
		deleteDialog: DeleteDialog;
		onCreateNameChange: (name: string) => void;
		onRenameNameChange: (name: string) => void;
		onCreateConfirm: () => void;
		onRenameConfirm: () => void;
		onDeleteConfirm: () => void;
		onCloseCreate: () => void;
		onCloseRename: () => void;
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
	{#snippet footer()}
		<Button variant="primary" onclick={onCreateConfirm}>Create</Button>
	{/snippet}
</Modal>

<Modal open={renameDialog.open} title="Rename" onclose={onCloseRename}>
	<Input
		value={renameDialog.newName}
		selectOnFocus={renameDialog.file?.isDir ? 'all' : 'basename'}
		oninput={(event) => onRenameNameChange(inputValue(event))}
		onkeydown={(event) => event.key === 'Enter' && onRenameConfirm()}
	/>
	{#snippet footer()}
		<Button variant="primary" onclick={onRenameConfirm}>Rename</Button>
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
