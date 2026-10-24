<script lang="ts">
	import type { FileInfo } from '#lib/api/files.js';
	import { Button, Input, Modal } from '#lib/components/ui/index.js';
	import type { BrowseOperation } from '#lib/browse/actions.svelte.js';

	type CreateDialog = { open: boolean; type: 'file' | 'directory'; name: string };
	type RenameDialog = { open: boolean; newName: string };
	type DeleteDialog =
		| { open: false; items: FileInfo[] }
		| { open: true; items: FileInfo[]; mode: 'trash' | 'permanent' };

	let {
		createDialog,
		renameDialog,
		deleteDialog,
		openWithFile = null,
		onCloseOpenWith,
		onCreateNameChange,
		onRenameNameChange,
		onCreateConfirm,
		onRenameConfirm,
		onDeleteConfirm,
		onCloseCreate,
		onCloseRename,
		onCloseDelete,
		operation = null
	}: {
		createDialog: CreateDialog;
		renameDialog: RenameDialog;
		deleteDialog: DeleteDialog;
		openWithFile?: FileInfo | null;
		onCloseOpenWith?: () => void;
		onCreateNameChange: (name: string) => void;
		onRenameNameChange: (name: string) => void;
		onCreateConfirm: () => void;
		onRenameConfirm: () => void;
		onDeleteConfirm: () => void;
		onCloseCreate: () => void;
		onCloseRename: () => void;
		onCloseDelete: () => void;
		operation?: BrowseOperation;
	} = $props();

	function inputValue(event: Event): string {
		return event.target instanceof HTMLInputElement ? event.target.value : '';
	}
</script>

{#if openWithFile}
	{#await import('#lib/components/desktop/OpenWithDialog.svelte') then { default: OpenWithDialog }}
		<OpenWithDialog open={true} path={openWithFile.path} onclose={() => onCloseOpenWith?.()} />
	{/await}
{/if}

<Modal
	open={createDialog.open}
	title={createDialog.type === 'file' ? 'New File' : 'New Folder'}
	onclose={onCloseCreate}
	busy={operation === 'creating'}
>
	<Input
		aria-label="Name"
		value={createDialog.name}
		disabled={operation === 'creating'}
		oninput={(event) => onCreateNameChange(inputValue(event))}
		placeholder={createDialog.type === 'file' ? 'untitled.txt' : 'New Folder'}
		onkeydown={(event) => event.key === 'Enter' && onCreateConfirm()}
	/>
	{#snippet footer()}
		<Button variant="primary" onclick={onCreateConfirm} disabled={operation !== null}>
			{operation === 'creating'
				? 'Creating...'
				: `Create ${createDialog.type === 'file' ? 'File' : 'Folder'}`}
		</Button>
	{/snippet}
</Modal>

<Modal
	open={renameDialog.open}
	title="Rename"
	onclose={onCloseRename}
	busy={operation === 'renaming'}
>
	<Input
		aria-label="New name"
		value={renameDialog.newName}
		disabled={operation === 'renaming'}
		oninput={(event) => onRenameNameChange(inputValue(event))}
		placeholder="New name"
		onkeydown={(event) => event.key === 'Enter' && onRenameConfirm()}
	/>
	{#snippet footer()}
		<Button variant="primary" onclick={onRenameConfirm} disabled={operation !== null}
			>{operation === 'renaming' ? 'Renaming...' : 'Rename'}</Button
		>
	{/snippet}
</Modal>

<Modal
	open={deleteDialog.open}
	title={deleteDialog.open && deleteDialog.mode === 'trash'
		? `Move to Trash (${deleteDialog.items.length})`
		: `Delete Permanently (${deleteDialog.items.length})`}
	onclose={onCloseDelete}
	busy={operation === 'deleting'}
>
	<div class="flex flex-col gap-3 text-sm text-text-secondary">
		{#if deleteDialog.open && deleteDialog.mode === 'permanent'}
			<p class="text-danger">This cannot be undone.</p>
		{/if}
		{#if deleteDialog.items.length > 0}
			<ul class="max-h-40 list-none overflow-auto rounded border border-border-secondary p-0">
				{#each deleteDialog.items as item (item.path)}
					<li class="border-b border-border-secondary px-3 py-2 last:border-b-0">
						<span class="block truncate text-text-primary" title={item.path}>{item.name}</span>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
	{#snippet footer()}
		<Button variant="danger" onclick={onDeleteConfirm} disabled={operation !== null}
			>{operation === 'deleting'
				? 'Working...'
				: deleteDialog.open && deleteDialog.mode === 'trash'
					? 'Move to Trash'
					: 'Delete Permanently'}</Button
		>
	{/snippet}
</Modal>
