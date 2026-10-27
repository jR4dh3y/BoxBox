<script lang="ts">
	/**
	 * The uploads in the floating panel: one row each, with a visible cancel or remove button.
	 */

	import type { UploadProgress as UploadProgressType } from '#lib/utils/upload.js';
	import { formatFileSize, formatPercentage } from '#lib/utils/format.js';
	import { X } from 'lucide-svelte';
	import { ProgressBar } from '#lib/components/ui/index.js';

	interface Props {
		uploads: UploadProgressType[];
		onCancel?: (uploadId: string) => void;
		onRemove?: (uploadId: string) => void;
	}

	let { uploads = [], onCancel, onRemove }: Props = $props();

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
				return 'Waiting';
			case 'uploading':
				return formatPercentage(upload.percentage, 0, false);
			case 'complete':
				return 'Done';
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

<ul class="m-0 max-h-[300px] list-none overflow-y-auto p-0" role="list">
	{#each uploads as upload (upload.uploadId)}
		<li
			class="flex items-center gap-2 border-b border-border-secondary py-2.5 pr-2 pl-4 last:border-b-0"
		>
			<div class="flex min-w-0 flex-1 flex-col gap-1.5">
				<div class="flex items-baseline justify-between gap-3 text-[13px]">
					<span class="min-w-0 truncate text-text-primary" title={upload.fileName}>
						{upload.fileName}
					</span>
					<span
						class="max-w-[50%] shrink-0 truncate text-xs {upload.status === 'error'
							? 'text-danger'
							: 'text-text-secondary'}"
						title={getStatusText(upload)}
					>
						{getStatusText(upload)}
					</span>
				</div>
				<ProgressBar
					value={upload.percentage}
					size="sm"
					variant={getProgressVariant(upload.status)}
				/>
				<span class="text-[11px] text-text-muted">
					{formatFileSize(upload.uploadedSize)} of {formatFileSize(upload.totalSize)}
				</span>
			</div>
			<button
				type="button"
				class="flex h-7 w-7 shrink-0 cursor-pointer items-center justify-center rounded text-text-secondary transition-colors hover:bg-surface-elevated hover:text-text-primary"
				onclick={() =>
					isTerminal(upload.status) ? onRemove?.(upload.uploadId) : onCancel?.(upload.uploadId)}
				aria-label={isTerminal(upload.status) ? 'Remove from list' : 'Cancel upload'}
				title={isTerminal(upload.status) ? 'Remove from list' : 'Cancel upload'}
			>
				<X size={14} />
			</button>
		</li>
	{/each}
</ul>
