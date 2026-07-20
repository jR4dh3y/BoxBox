<script lang="ts">
	import type { FileInfo } from '$lib/api/files';
	import { formatFileDate, formatFileSize } from '$lib/utils/format';
	import { Button, Input, Modal } from '$lib/components/ui';

	type CreateDialog = { open: boolean; type: 'file' | 'directory'; name: string };
	type RenameDialog = { open: boolean; newName: string };
	type DeleteDialog = { open: boolean; items: FileInfo[] };
	type PropertiesDialog = { open: boolean; file: FileInfo | null };

	let {
		createDialog,
		renameDialog,
		deleteDialog,
		propertiesDialog,
		onCreateNameChange,
		onRenameNameChange,
		onCreateConfirm,
		onRenameConfirm,
		onDeleteConfirm,
		onCloseCreate,
		onCloseRename,
		onCloseDelete,
		onCloseProperties
	}: {
		createDialog: CreateDialog;
		renameDialog: RenameDialog;
		deleteDialog: DeleteDialog;
		propertiesDialog: PropertiesDialog;
		onCreateNameChange: (name: string) => void;
		onRenameNameChange: (name: string) => void;
		onCreateConfirm: () => void;
		onRenameConfirm: () => void;
		onDeleteConfirm: () => void;
		onCloseCreate: () => void;
		onCloseRename: () => void;
		onCloseDelete: () => void;
		onCloseProperties: () => void;
	} = $props();

	function inputValue(event: Event): string {
		return (event.target as HTMLInputElement).value;
	}
</script>

<Modal
	open={createDialog.open}
	title={createDialog.type === 'file' ? 'New File' : 'New Folder'}
	onclose={onCloseCreate}
>
	<div class="flex flex-col gap-4">
		<p class="text-sm text-text-secondary">
			Enter a name for the new {createDialog.type === 'file' ? 'file' : 'folder'}:
		</p>
		<Input
			value={createDialog.name}
			oninput={(event) => onCreateNameChange(inputValue(event))}
			placeholder={createDialog.type === 'file' ? 'untitled.txt' : 'New Folder'}
			onkeydown={(event) => event.key === 'Enter' && onCreateConfirm()}
		/>
	</div>
	{#snippet footer()}
		<Button variant="secondary" onclick={onCloseCreate}>Cancel</Button>
		<Button variant="primary" onclick={onCreateConfirm}>
			Create {createDialog.type === 'file' ? 'File' : 'Folder'}
		</Button>
	{/snippet}
</Modal>

<Modal open={renameDialog.open} title="Rename" onclose={onCloseRename}>
	<div class="flex flex-col gap-4">
		<p class="text-sm text-text-secondary">Enter a new name:</p>
		<Input
			value={renameDialog.newName}
			oninput={(event) => onRenameNameChange(inputValue(event))}
			placeholder="New name"
			onkeydown={(event) => event.key === 'Enter' && onRenameConfirm()}
		/>
	</div>
	{#snippet footer()}
		<Button variant="secondary" onclick={onCloseRename}>Cancel</Button>
		<Button variant="primary" onclick={onRenameConfirm}>Rename</Button>
	{/snippet}
</Modal>

<Modal open={deleteDialog.open} title="Delete" onclose={onCloseDelete}>
	<div class="flex flex-col gap-3 text-sm text-text-secondary">
		<p>
			Delete {deleteDialog.items.length}
			{deleteDialog.items.length === 1 ? 'item' : 'items'}?
		</p>
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
		<Button variant="secondary" onclick={onCloseDelete}>Cancel</Button>
		<Button variant="danger" onclick={onDeleteConfirm}>Delete</Button>
	{/snippet}
</Modal>

<Modal open={propertiesDialog.open} title="Properties" onclose={onCloseProperties}>
	{#if propertiesDialog.file}
		{@const file = propertiesDialog.file}
		<div class="flex flex-col gap-3 text-sm">
			<div class="flex justify-between">
				<span class="text-text-secondary">Name:</span>
				<span class="font-medium text-text-primary">{file.name}</span>
			</div>
			<div class="flex justify-between">
				<span class="text-text-secondary">Type:</span>
				<span class="text-text-primary">{file.isDir ? 'Folder' : file.mimeType || 'File'}</span>
			</div>
			<div class="flex justify-between">
				<span class="text-text-secondary">Path:</span>
				<span class="break-all text-text-primary">{file.path}</span>
			</div>
			{#if !file.isDir}
				<div class="flex justify-between">
					<span class="text-text-secondary">Size:</span>
					<span class="text-text-primary">{formatFileSize(file.size)}</span>
				</div>
			{/if}
			<div class="flex justify-between">
				<span class="text-text-secondary">Modified:</span>
				<span class="text-text-primary">{formatFileDate(file.modTime)}</span>
			</div>
			<div class="flex justify-between">
				<span class="text-text-secondary">Permissions:</span>
				<span class="font-mono text-text-primary">{file.permissions}</span>
			</div>
		</div>
	{/if}
	{#snippet footer()}
		<Button variant="secondary" onclick={onCloseProperties}>Close</Button>
	{/snippet}
</Modal>
