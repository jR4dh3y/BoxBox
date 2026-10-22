<script lang="ts">
	import { Copy, X } from 'lucide-svelte';
	import type { FileInfo } from '#lib/api/files.js';
	import { formatFileSize } from '#lib/utils/format.js';
	import { getFileTypeDescription } from '#lib/utils/fileTypes.js';
	import { summarizeSelection, parentFolder, fullFileDate } from '#lib/browse/details.js';
	import { copyText } from '#lib/utils/clipboard.js';
	import { toastStore } from '#lib/stores/toast.svelte.js';
	import FileThumbnail from './FileThumbnail.svelte';
	import type { BrowsePaneRuntime } from '#lib/browse/pane.svelte.js';
	import { isLocalDesktop } from '#lib/api/desktop.js';
	import type { FileContextAction } from '#lib/utils/fileContextMenu.js';

	let { runtime, onClose }: { runtime: BrowsePaneRuntime | null; onClose: () => void } = $props();
	const files = $derived(
		runtime?.data.displayItems.filter((file) => runtime?.pane.selection.paths.has(file.path)) ?? []
	);
	const path = $derived(runtime?.pane.location.path ?? '');
	const itemCount = $derived(runtime?.data.itemCount ?? 0);
	const readOnly = $derived(runtime?.data.isReadOnly ?? true);
	function onOpen(file: FileInfo) {
		runtime?.actions.openFile(file);
	}
	function onNavigate(path: string) {
		runtime?.pane.location.navigate(path);
	}
	function onAction(action: FileContextAction, files: FileInfo[]) {
		void runtime?.actions.handleContextMenuAction(action, files);
	}
	function onClear() {
		runtime?.pane.selection.clear();
	}

	const summary = $derived(summarizeSelection(files));
	const single = $derived(files.length === 1 ? files[0] : null);
	const control =
		'inline-flex min-h-8 items-center justify-center gap-2 rounded border border-border-primary bg-surface-secondary px-3 py-1.5 text-xs text-text-primary transition-colors hover:bg-surface-elevated focus-visible:outline-2 focus-visible:outline-accent';
	async function copyPath() {
		try {
			await copyText(single?.path ?? path);
			toastStore.success('Location copied');
		} catch {
			toastStore.error('Could not copy the location');
		}
	}
</script>

<aside
	class="flex h-full min-h-0 w-full flex-col border-l border-border-secondary bg-surface-primary"
	aria-label="Selection details"
>
	<header
		class="flex h-11 shrink-0 items-center justify-between border-b border-border-secondary px-4"
	>
		<h2 class="flex items-center gap-2 text-[13px] font-medium text-text-primary">Details</h2>
		<button
			type="button"
			class="rounded p-1 text-text-secondary hover:bg-surface-elevated hover:text-text-primary focus-visible:outline-2 focus-visible:outline-accent"
			aria-label="Close details"
			onclick={onClose}><X size={16} /></button
		>
	</header>
	<div class="min-h-0 flex-1 overflow-y-auto p-4">
		{#if single}
			<div
				class="mb-4 flex h-40 items-center justify-center overflow-hidden rounded-lg bg-surface-secondary"
			>
				<FileThumbnail file={single} width={288} height={160} />
			</div>
			<h3 class="text-sm font-medium break-words text-text-primary">{single.name}</h3>
			<p class="mt-1 text-xs text-text-secondary">
				{single.isDir ? 'Folder' : getFileTypeDescription(single.name)}
			</p>
			<button type="button" class="{control} mt-4 w-full" onclick={() => onOpen(single)}
				>{single.isDir ? 'Open folder' : 'Open'}</button
			>
			<dl class="mt-6 space-y-4 text-xs">
				<div>
					<dt class="text-text-secondary">Location</dt>
					<dd class="mt-1 flex items-start gap-2">
						<button
							type="button"
							class="min-w-0 flex-1 text-left break-all text-text-primary underline-offset-4 hover:underline focus-visible:outline-2 focus-visible:outline-accent"
							onclick={() => onNavigate(parentFolder(single.path))}
							>{parentFolder(single.path) || 'This server'}</button
						><button
							type="button"
							class="shrink-0 rounded p-1 text-text-secondary hover:text-text-primary focus-visible:outline-2 focus-visible:outline-accent"
							title="Copy location"
							aria-label="Copy location"
							onclick={() => void copyPath()}><Copy size={13} /></button
						>
					</dd>
				</div>
				{#if !single.isDir}<div>
						<dt class="text-text-secondary">Size</dt>
						<dd class="mt-1 text-text-primary">{formatFileSize(single.size)}</dd>
					</div>{/if}
				<div>
					<dt class="text-text-secondary">Modified</dt>
					<dd class="mt-1 text-text-primary">{fullFileDate(single.modTime)}</dd>
				</div>
				{#if single.permissions}<div>
						<dt class="text-text-secondary">Permissions</dt>
						<dd class="mt-1 font-mono text-text-primary">{single.permissions}</dd>
					</div>{/if}
				<div>
					<dt class="text-text-secondary">Access</dt>
					<dd class="mt-1 text-text-primary">{readOnly ? 'Read only' : 'Writable mount'}</dd>
				</div>
			</dl>
		{:else if files.length > 1}
			<h3 class="text-sm font-medium text-text-primary">{summary.count} items selected</h3>
			<p class="mt-2 text-xs text-text-secondary">
				{summary.files} files · {summary.folders} folders
			</p>
			<dl class="mt-5 text-xs">
				<dt class="text-text-secondary">
					{summary.folders ? 'Size of selected files (folders excluded)' : 'Size'}
				</dt>
				<dd class="mt-1 text-text-primary">{formatFileSize(summary.bytes)}</dd>
			</dl>
			<ul
				class="mt-5 max-h-52 space-y-2 overflow-y-auto text-xs text-text-secondary"
				aria-label="Selected items"
			>
				{#each files as file (file.path)}<li class="truncate" title={file.name}>
						{file.name}
					</li>{/each}
			</ul>
			<button type="button" class="{control} mt-4 w-full" onclick={onClear}>Clear selection</button>
		{:else}
			<p class="text-xs break-all text-text-primary">{path || 'This server'}</p>
			<p class="mt-1 text-xs text-text-secondary">{itemCount} items</p>
		{/if}
		{#if files.length}
			<div class="mt-6 grid grid-cols-2 gap-2 border-t border-border-secondary pt-4">
				<button type="button" class={control} onclick={() => onAction('copy', files)}>Copy</button>
				{#if summary.files === files.length}<button
						type="button"
						class={control}
						onclick={() => onAction('download', files)}>Download</button
					>{/if}
				{#if !readOnly}
					{#if single}<button
							type="button"
							class={control}
							onclick={() => onAction('rename', files)}>Rename</button
						>{/if}
					<button
						type="button"
						class="{control} text-danger"
						onclick={() => onAction('delete', files)}
						>{isLocalDesktop() ? 'Trash' : 'Delete'}</button
					>
				{/if}
			</div>
		{/if}
	</div>
</aside>
