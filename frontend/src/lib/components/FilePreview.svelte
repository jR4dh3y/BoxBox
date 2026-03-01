<script lang="ts">
	/**
	 * FilePreview - Modal component for previewing files
	 */
	import { X, Download, Maximize2, Minimize2, ChevronLeft, ChevronRight } from 'lucide-svelte';
	import type { FileInfo } from '$lib/api/files';
	import { getPreviewUrl, getDownloadUrl } from '$lib/api/files';
	import { getPreviewType, type PreviewType } from '$lib/utils/fileTypes';
	import { formatFileSize } from '$lib/utils/format';
	import { Button } from '$lib/components/ui';
	import VideoPreview from './preview/VideoPreview.svelte';
	import AudioPreview from './preview/AudioPreview.svelte';
	import ImagePreview from './preview/ImagePreview.svelte';
	import CodePreview from './preview/CodePreview.svelte';
	import PdfPreview from './preview/PdfPreview.svelte';

	interface Props {
		file: FileInfo | null;
		allFiles?: FileInfo[];
		onNavigate?: (file: FileInfo) => void;
		onClose: () => void;
	}

	let { file, allFiles = [], onNavigate, onClose }: Props = $props();

	let isFullscreen = $state(false);

	const previewType = $derived<PreviewType>(file ? getPreviewType(file.name) : 'unsupported');
	const previewUrl = $derived(file ? getPreviewUrl(file.path) : '');
	const downloadUrl = $derived(file ? getDownloadUrl(file.path) : '');
	const currentIndex = $derived(file ? allFiles.findIndex((item) => item.path === file.path) : -1);
	const hasPrevious = $derived(currentIndex > 0);
	const hasNext = $derived(currentIndex >= 0 && currentIndex < allFiles.length - 1);

	function isInteractiveTarget(target: EventTarget | null): boolean {
		const element = target instanceof HTMLElement ? target : null;
		if (!element) return false;
		return !!element.closest('input, textarea, select, button, [contenteditable="true"], audio, video');
	}

	function navigatePrevious() {
		if (!hasPrevious) return;
		onNavigate?.(allFiles[currentIndex - 1]);
	}

	function navigateNext() {
		if (!hasNext) return;
		onNavigate?.(allFiles[currentIndex + 1]);
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			if (isFullscreen) {
				isFullscreen = false;
			} else {
				onClose();
			}
			return;
		}

		if (isInteractiveTarget(event.target)) {
			return;
		}

		if (event.key === 'ArrowLeft') {
			event.preventDefault();
			navigatePrevious();
			return;
		}

		if (event.key === 'ArrowRight') {
			event.preventDefault();
			navigateNext();
		}
	}

	function handleBackdropClick(event: MouseEvent) {
		if (event.target === event.currentTarget) {
			onClose();
		}
	}

	function toggleFullscreen() {
		isFullscreen = !isFullscreen;
	}

	function handleDownload() {
		if (downloadUrl) {
			window.open(downloadUrl, '_blank');
		}
	}

	const headerBtnClass =
		'w-8 h-8 flex items-center justify-center bg-transparent border-none rounded text-text-secondary cursor-pointer transition-all duration-100 hover:bg-surface-elevated hover:text-text-primary disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-text-secondary';
</script>

<svelte:window onkeydown={handleKeydown} />

{#if file}
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class="fixed inset-0 bg-black/85 flex items-center justify-center z-[1000] {isFullscreen ? 'p-0' : 'p-10'}"
		onclick={handleBackdropClick}
		onkeydown={handleKeydown}
		role="dialog"
		aria-modal="true"
		aria-label="File preview"
		tabindex="-1"
	>
		<div
			class="flex flex-col w-full h-full bg-surface-primary rounded-lg overflow-hidden shadow-2xl {isFullscreen
				? 'max-w-none max-h-none rounded-none'
				: 'max-w-[1200px] max-h-[90vh]'}"
		>
			<!-- Header -->
			<header class="flex items-center justify-between px-4 py-3 bg-surface-secondary border-b border-border-primary shrink-0">
				<div class="flex items-center gap-3 min-w-0">
					<span class="text-sm font-medium text-text-primary overflow-hidden text-ellipsis whitespace-nowrap" title={file.name}>
						{file.name}
					</span>
					<span class="text-xs text-text-secondary shrink-0">{formatFileSize(file.size)}</span>
				</div>
				<div class="flex items-center gap-1">
					<button type="button" class={headerBtnClass} onclick={navigatePrevious} disabled={!hasPrevious} title="Previous file">
						<ChevronLeft size={18} />
					</button>
					<button type="button" class={headerBtnClass} onclick={navigateNext} disabled={!hasNext} title="Next file">
						<ChevronRight size={18} />
					</button>
					<button type="button" class={headerBtnClass} onclick={handleDownload} title="Download">
						<Download size={18} />
					</button>
					<button
						type="button"
						class={headerBtnClass}
						onclick={toggleFullscreen}
						title={isFullscreen ? 'Exit fullscreen' : 'Fullscreen'}
					>
						{#if isFullscreen}
							<Minimize2 size={18} />
						{:else}
							<Maximize2 size={18} />
						{/if}
					</button>
					<button type="button" class="{headerBtnClass} hover:bg-danger hover:text-white" onclick={onClose} title="Close">
						<X size={18} />
					</button>
				</div>
			</header>

			<!-- Content -->
			<main class="flex-1 overflow-auto flex items-center justify-center">
				{#if previewType === 'video'}
					<VideoPreview url={previewUrl} filename={file.name} downloadUrl={downloadUrl} sizeBytes={file.size} />
				{:else if previewType === 'audio'}
					<AudioPreview url={previewUrl} filename={file.name} />
				{:else if previewType === 'image'}
					<ImagePreview url={previewUrl} filename={file.name} />
				{:else if previewType === 'pdf'}
					<PdfPreview url={previewUrl} filename={file.name} />
				{:else if previewType === 'code' || previewType === 'text'}
					<CodePreview url={previewUrl} filename={file.name} />
				{:else}
					<div class="flex flex-col items-center gap-4 text-text-secondary text-sm">
						<p>Preview not available for this file type</p>
						<Button variant="primary" onclick={handleDownload}>
							<Download size={20} />
							Download File
						</Button>
					</div>
				{/if}
			</main>
		</div>
	</div>
{/if}
