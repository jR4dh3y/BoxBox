<script lang="ts">
	/**
	 * UploadDropzone component with drag-drop support
	 * Triggers chunked upload on file drop
	 */
	import { Upload } from 'lucide-svelte';

	interface Props {
		disabled?: boolean;
		accept?: string;
		multiple?: boolean;
		onFilesSelected?: (files: File[]) => void;
	}

	let { disabled = false, accept = '', multiple = true, onFilesSelected }: Props = $props();

	let isDragOver = $state(false);
	let fileInput: HTMLInputElement;

	function handleDragOver(event: DragEvent) {
		if (disabled) return;
		event.preventDefault();
		event.stopPropagation();
		isDragOver = true;
	}

	function handleDragLeave(event: DragEvent) {
		event.preventDefault();
		event.stopPropagation();
		isDragOver = false;
	}

	function handleDrop(event: DragEvent) {
		event.preventDefault();
		event.stopPropagation();
		isDragOver = false;

		if (disabled) return;

		const files = event.dataTransfer?.files;
		if (files && files.length > 0) {
			const fileArray = multiple ? Array.from(files) : [files[0]];
			onFilesSelected?.(fileArray);
		}
	}

	function handleFileChange(event: Event) {
		const target = event.target as HTMLInputElement;
		const files = target.files;

		if (files && files.length > 0) {
			const fileArray = Array.from(files);
			onFilesSelected?.(fileArray);
		}

		target.value = '';
	}

	function openFilePicker() {
		if (!disabled) {
			fileInput?.click();
		}
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			openFilePicker();
		}
	}
</script>

<div
	class="flex flex-col items-center justify-center p-8 border-2 border-dashed rounded-lg cursor-pointer transition-all min-h-[150px]
		{isDragOver ? 'border-accent bg-accent/10 border-solid' : 'border-border-primary bg-surface-secondary hover:border-text-muted hover:bg-surface-tertiary'}
		{disabled ? 'opacity-50 cursor-not-allowed' : ''}
		focus:outline-2 focus:outline-accent focus:outline-offset-2"
	ondragover={handleDragOver}
	ondragleave={handleDragLeave}
	ondrop={handleDrop}
	onclick={openFilePicker}
	onkeydown={handleKeyDown}
	tabindex={disabled ? -1 : 0}
	role="button"
	aria-label="Upload files by clicking or dragging"
	aria-disabled={disabled}
>
	<input
		bind:this={fileInput}
		type="file"
		{accept}
		{multiple}
		{disabled}
		onchange={handleFileChange}
		class="absolute w-px h-px p-0 -m-px overflow-hidden whitespace-nowrap border-0"
		style="clip: rect(0, 0, 0, 0);"
		aria-hidden="true"
		tabindex="-1"
	/>

	<div class="flex flex-col items-center gap-3 text-center">
		<div class="flex items-center justify-center w-12 h-12 {isDragOver ? 'text-accent' : 'text-text-muted'}">
			<Upload size={48} />
		</div>

		<div class="flex flex-col gap-1">
			{#if isDragOver}
				<span class="text-sm text-text-primary">Drop files here</span>
			{:else}
				<span class="text-sm text-text-secondary">
					<span class="text-accent font-medium">Click to upload</span> or drag and drop
				</span>
				<span class="text-xs text-text-muted">
					{#if accept}
						Accepted: {accept}
					{:else}
						Any file type
					{/if}
				</span>
			{/if}
		</div>
	</div>
</div>
