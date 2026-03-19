<script lang="ts">
	/**
	 * UploadProgress component showing active uploads
	 * Displays progress bars with icon that transforms to X on hover
	 */

	import type { UploadProgress as UploadProgressType } from '$lib/utils/upload';
	import { formatFileSize, formatPercentage } from '$lib/utils/format';
	import { X, Upload } from 'lucide-svelte';
	import { Badge, Button, ProgressBar } from '$lib/components/ui';

	interface Props {
		uploads: UploadProgressType[];
		onCancel?: (uploadId: string) => void;
		onRemove?: (uploadId: string) => void;
		onClearCompleted?: () => void;
		showCompleted?: boolean;
	}

	let {
		uploads = [],
		onCancel,
		onRemove,
		onClearCompleted,
		showCompleted = true
	}: Props = $props();

	const filteredUploads = $derived(
		showCompleted
			? uploads
			: uploads.filter((u) => u.status !== 'complete' && u.status !== 'cancelled')
	);

	const activeCount = $derived(
		uploads.filter((u) => u.status === 'pending' || u.status === 'uploading').length
	);
	const completedCount = $derived(
		uploads.filter(
			(u) => u.status === 'complete' || u.status === 'error' || u.status === 'cancelled'
		).length
	);

	function getProgressVariant(
		status: UploadProgressType['status']
	): 'default' | 'success' | 'warning' | 'danger' {
		switch (status) {
			case 'complete':
				return 'success';
			case 'error':
				return 'danger';
			default:
				return 'default';
		}
	}

	function getStatusText(upload: UploadProgressType): string {
		switch (upload.status) {
			case 'pending':
				return 'Waiting...';
			case 'uploading':
				return formatPercentage(upload.percentage, 0, false);
			case 'complete':
				return 'Complete';
			case 'error':
				return upload.error || 'Failed';
			case 'cancelled':
				return 'Cancelled';
			default:
				return '';
		}
	}

	function isTerminal(status: UploadProgressType['status']): boolean {
		return status === 'complete' || status === 'error' || status === 'cancelled';
	}
</script>

{#if filteredUploads.length > 0}
	<div class="overflow-hidden rounded-lg border border-border-primary bg-surface-secondary">
		<div
			class="flex items-center justify-between border-b border-border-secondary bg-surface-primary px-4 py-3"
		>
			<h3 class="m-0 flex items-center gap-2 text-sm font-semibold text-text-primary">
				Uploads
				{#if activeCount > 0}
					<Badge variant="info">{activeCount} active</Badge>
				{/if}
			</h3>
			{#if completedCount > 0 && onClearCompleted}
				<Button variant="ghost" size="sm" onclick={onClearCompleted}>Clear done</Button>
			{/if}
		</div>

		<ul class="m-0 max-h-[300px] list-none overflow-y-auto p-0" role="list">
			{#each filteredUploads as upload (upload.uploadId)}
				<li
					class="border-b border-border-secondary px-4 py-3 transition-all last:border-b-0 hover:bg-surface-tertiary"
				>
					<div class="flex items-stretch gap-3">
						<!-- Icon that transforms to X on hover -->
						<button
							type="button"
							class="group flex w-16 shrink-0 cursor-pointer items-center justify-center rounded border-none bg-surface-elevated text-text-secondary transition-all hover:bg-danger/20 hover:text-danger"
							onclick={() =>
								isTerminal(upload.status)
									? onRemove?.(upload.uploadId)
									: onCancel?.(upload.uploadId)}
							aria-label={isTerminal(upload.status) ? 'Remove from list' : 'Cancel upload'}
						>
							<span class="group-hover:hidden">
								<Upload size={20} />
							</span>
							<span class="hidden group-hover:block">
								<X size={20} />
							</span>
						</button>

						<div class="flex min-w-0 flex-1 flex-col gap-1 py-0.5">
							<div class="flex items-center justify-between gap-2">
								<span
									class="overflow-hidden text-sm font-medium text-ellipsis whitespace-nowrap text-text-primary"
									title={upload.fileName}
								>
									{upload.fileName}
								</span>
								<span class="shrink-0 text-xs text-text-muted">
									{formatFileSize(upload.totalSize)}
								</span>
							</div>
							<div class="text-xs text-text-muted">
								{#if upload.status === 'uploading'}
									Chunk {upload.currentChunk + 1}/{upload.totalChunks}
								{:else}
									{formatFileSize(upload.uploadedSize)} / {formatFileSize(upload.totalSize)}
								{/if}
							</div>

							<!-- Progress bar with inline status -->
							<div class="flex items-center gap-3">
								<div class="flex-1">
									<ProgressBar
										value={upload.percentage}
										size="sm"
										variant={getProgressVariant(upload.status)}
									/>
								</div>
								<span
									class="shrink-0 text-[11px] {upload.status === 'complete'
										? 'text-success'
										: ''} {upload.status === 'error' ? 'text-danger' : ''} {upload.status ===
									'uploading'
										? 'text-accent'
										: 'text-text-muted'}"
								>
									{getStatusText(upload)}
								</span>
							</div>
						</div>
					</div>
				</li>
			{/each}
		</ul>
	</div>
{/if}
