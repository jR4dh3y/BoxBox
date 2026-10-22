<script lang="ts">
	import type { FileInfo } from '#lib/api/files.js';
	import { formatFileDate, formatFileSize } from '#lib/utils/format.js';
	import { getFileTypeDescription } from '#lib/utils/fileTypes.js';
	import { Button, Input, Modal } from '#lib/components/ui/index.js';
	import type { BrowseOperation } from '#lib/browse/actions.svelte.js';

	type CreateDialog = { open: boolean; type: 'file' | 'directory'; name: string };
	import type { RenameDialog } from '#lib/browse/actions.svelte.js';
	type DeleteDialog = { open: boolean; items: FileInfo[] };
	type PropertiesDialog = { open: boolean; file: FileInfo | null };

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

	const deleteCount = $derived(deleteDialog.items.length);
	const deleteFirst = $derived(deleteDialog.items[0]);
</script>

{#if openWithFile}
	{#await import('#lib/components/desktop/OpenWithDialog.svelte') then { default: OpenWithDialog }}
		<OpenWithDialog open={true} path={openWithFile.path} onclose={() => onCloseOpenWith?.()} />
	{/await}
{/if}

<Modal
	open={createDialog.open}
	title={createDialog.type === 'file' ? 'New file' : 'New folder'}
	onclose={onCloseCreate}
	busy={operation === 'creating'}
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

<Modal open={deleteDialog.open} title="Delete" onclose={onCloseDelete}>
	<div class="flex flex-col gap-3 text-[13px] text-text-secondary">
		{#if deleteCount === 1 && deleteFirst}
			<p class="m-0 break-all">
				<span class="text-text-primary">{deleteFirst.name}</span> will be deleted permanently.
			</p>
		{:else}
			<p class="m-0">{deleteCount} items will be deleted permanently.</p>
			<ul class="m-0 max-h-40 list-none overflow-auto p-0">
				{#each deleteDialog.items as item (item.path)}
					<li class="truncate py-0.5 text-text-primary" title={item.path}>{item.name}</li>
				{/each}
			</ul>
		{/if}
	</div>
	{#snippet footer()}
		<Button variant="danger" onclick={onDeleteConfirm}>Delete</Button>
	{/snippet}
</Modal>

<Modal open={propertiesDialog.open} title="Properties" onclose={onCloseProperties}>
	{#if propertiesDialog.file}
		{@const file = propertiesDialog.file}
		<dl class="m-0 grid grid-cols-[5.5rem_minmax(0,1fr)] gap-x-4 gap-y-2.5 text-[13px]">
			<dt class="text-text-secondary">Name</dt>
			<dd class="m-0 break-all text-text-primary">{file.name}</dd>
			<dt class="text-text-secondary">Type</dt>
			<dd class="m-0 text-text-primary">
				{file.isDir ? 'Folder' : getFileTypeDescription(file.name)}
			</dd>
			<dt class="text-text-secondary">Location</dt>
			<dd class="m-0 break-all text-text-primary">
				{file.path.slice(0, Math.max(0, file.path.lastIndexOf('/'))) || '/'}
			</dd>
			{#if !file.isDir}
				<dt class="text-text-secondary">Size</dt>
				<dd class="m-0 text-text-primary">{formatFileSize(file.size)}</dd>
			{/if}
			<dt class="text-text-secondary">Modified</dt>
			<dd class="m-0 text-text-primary">{formatFileDate(file.modTime)}</dd>
			<dt class="text-text-secondary">Permissions</dt>
			<dd class="m-0 font-mono text-text-primary">{file.permissions}</dd>
		</dl>
	{/if}
</Modal>
