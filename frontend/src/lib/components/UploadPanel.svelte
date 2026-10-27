<script lang="ts">
	/**
	 * UploadPanel - Google Drive style upload panel at the bottom of the notification column
	 * Minimizable, manually closed
	 */
	import { uploadStore } from '#lib/stores/upload.svelte.js';
	import UploadProgress from '#lib/components/UploadProgress.svelte';
	import { ChevronDown, ChevronUp, X } from 'lucide-svelte';
	import { slide } from 'svelte/transition';

	let announcement = $state('');
	// This history is not rendered; only terminal successes update the live region.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const announced = new Set<string>();
	$effect(() => {
		const completed: string[] = [];
		for (const upload of uploadStore.uploads) {
			if (upload.status !== 'complete' || announced.has(upload.uploadId)) continue;
			announced.add(upload.uploadId);
			completed.push(upload.fileName);
		}
		if (completed.length > 0) {
			announcement = `${completed.join(', ')} uploaded. ${announced.size} complete.`;
		}
	});

	/** Whether the panel is minimized */
	let isMinimized = $state(false);

	/** Whether the panel is visible (user can close it) */
	let isVisible = $state(true);

	/** Track if we should auto-show on new uploads */
	let lastUploadCount = $state(0);

	// Auto-show panel when new uploads are added
	$effect(() => {
		const currentCount = uploadStore.uploads.length;
		if (currentCount > lastUploadCount && !isVisible) {
			isVisible = true;
			isMinimized = false;
		}
		lastUploadCount = currentCount;
	});

	function handleToggleMinimize() {
		isMinimized = !isMinimized;
	}

	function handleClose() {
		isVisible = false;
	}

	function handleCancel(uploadId: string) {
		uploadStore.cancel(uploadId);
	}

	function handleRemove(uploadId: string) {
		uploadStore.remove(uploadId);
	}

	function handleClearCompleted() {
		uploadStore.clearFinished();
	}

	// Header text shows upload status
	const headerText = $derived.by(() => {
		const active = uploadStore.activeCount;
		const total = uploadStore.uploads.length;

		if (active === 0 && total === 0) {
			return 'Uploads';
		}

		if (active === 0) {
			return `${total} upload${total !== 1 ? 's' : ''} complete`;
		}

		if (total === 1) {
			return 'Uploading 1 file';
		}

		return `Uploading ${active} of ${total}`;
	});

	// Only show if visible AND has uploads
	const shouldShow = $derived(isVisible && uploadStore.hasUploads);
</script>

<span class="sr-only" role="status" aria-live="polite">{announcement}</span>

{#if shouldShow}
	<div
		class="pointer-events-auto overflow-hidden rounded-lg border border-border-primary bg-surface-secondary shadow-2xl"
		role="region"
		aria-label="Upload progress"
	>
		<!-- Header -->
		<div
			class="flex h-11 cursor-pointer items-center justify-between border-b border-border-secondary bg-surface-primary pr-2 pl-4 select-none"
			onclick={handleToggleMinimize}
			onkeydown={(e) => {
				if (e.target === e.currentTarget && (e.key === 'Enter' || e.key === ' ')) {
					e.preventDefault();
					handleToggleMinimize();
				}
			}}
			role="button"
			tabindex="0"
		>
			<span class="text-[13px] font-medium text-text-primary">{headerText}</span>
			<div class="flex items-center gap-1">
				{#if uploadStore.uploads.some((upload) => upload.status === 'complete' || upload.status === 'error' || upload.status === 'cancelled')}
					<button
						type="button"
						class="rounded px-2 py-1 text-xs text-text-secondary transition-colors hover:bg-surface-elevated hover:text-text-primary"
						onclick={(e) => {
							e.stopPropagation();
							handleClearCompleted();
						}}
					>
						Clear finished
					</button>
				{/if}
				<!-- Minimize/Expand button -->
				<button
					type="button"
					class="rounded p-1 text-text-secondary transition-colors hover:bg-surface-elevated hover:text-text-primary"
					onclick={(e) => {
						e.stopPropagation();
						handleToggleMinimize();
					}}
					aria-label={isMinimized ? 'Expand' : 'Minimize'}
				>
					{#if isMinimized}
						<ChevronUp size={16} />
					{:else}
						<ChevronDown size={16} />
					{/if}
				</button>

				<!-- Close button -->
				<button
					type="button"
					class="rounded p-1 text-text-secondary transition-colors hover:bg-surface-elevated hover:text-text-primary"
					onclick={(e) => {
						e.stopPropagation();
						handleClose();
					}}
					aria-label="Close upload panel"
				>
					<X size={16} />
				</button>
			</div>
		</div>

		<!-- Content (collapsible) -->
		{#if !isMinimized}
			<div transition:slide={{ duration: 200 }}>
				<UploadProgress
					uploads={uploadStore.uploads}
					onCancel={handleCancel}
					onRemove={handleRemove}
				/>
			</div>
		{/if}
	</div>
{/if}
