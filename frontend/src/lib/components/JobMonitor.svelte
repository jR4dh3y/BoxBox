<script lang="ts">
	/**
	 * JobMonitor component showing background jobs
	 * Displays progress, status, and cancel option
	 */

	import type { Job } from '$lib/api/jobs';
	import { isJobActive, isJobTerminal } from '$lib/api/jobs';
	import { formatPercentage, formatFileDate } from '$lib/utils/format';
	import { X, Copy, FolderInput, Trash2, Settings } from 'lucide-svelte';
	import { Badge, ProgressBar, Button } from '$lib/components/ui';

	interface Props {
		jobs: Job[];
		onCancel?: (jobId: string) => void;
		onRemove?: (jobId: string) => void;
		onClearCompleted?: () => void;
		showCompleted?: boolean;
		maxDisplay?: number;
	}

	let {
		jobs = [],
		onCancel,
		onRemove,
		onClearCompleted,
		showCompleted = true,
		maxDisplay = 10
	}: Props = $props();

	const filteredJobs = $derived(
		(showCompleted ? jobs : jobs.filter((j) => !isJobTerminal(j))).slice(0, maxDisplay)
	);

	const activeCount = $derived(jobs.filter(isJobActive).length);
	const completedCount = $derived(jobs.filter(isJobTerminal).length);

	function getStatusText(job: Job): string {
		switch (job.state) {
			case 'pending':
				return 'Waiting...';
			case 'running':
				return `${formatPercentage(job.progress, 0, false)}`;
			case 'completed':
				return 'Completed';
			case 'failed':
				return job.error || 'Failed';
			case 'cancelled':
				return 'Cancelled';
			default:
				return '';
		}
	}

	function getFileName(path: string): string {
		const parts = path.split('/');
		return parts[parts.length - 1] || path;
	}

	function getFolderPath(path: string): string {
		const parts = path.split('/');
		parts.pop(); // Remove filename
		return parts.join('/') || '/';
	}
</script>

{#if filteredJobs.length > 0}
	<div class="overflow-hidden rounded-lg border border-border-primary bg-surface-secondary">
		<div
			class="flex items-center justify-between border-b border-border-secondary bg-surface-primary px-4 py-3"
		>
			<h3 class="m-0 flex items-center gap-2 text-sm font-semibold text-text-primary">
				Background Jobs
				{#if activeCount > 0}
					<Badge variant="info">{activeCount} active</Badge>
				{/if}
			</h3>
			{#if completedCount > 0 && onClearCompleted}
				<Button variant="ghost" size="sm" onclick={onClearCompleted}>Clear done</Button>
			{/if}
		</div>

		<ul class="m-0 max-h-[400px] list-none overflow-y-auto p-0" role="list">
			{#each filteredJobs as job (job.id)}
				<li
					class="border-b border-border-secondary px-4 py-3 transition-all last:border-b-0 hover:bg-surface-tertiary"
				>
					<div class="flex items-stretch gap-3">
						<!-- Icon that transforms to X on hover - stretches to match content -->
						<button
							type="button"
							class="group flex w-16 shrink-0 cursor-pointer items-center justify-center rounded border-none bg-surface-elevated text-text-secondary transition-all hover:bg-danger/20 hover:text-danger"
							onclick={() => (isJobTerminal(job) ? onRemove?.(job.id) : onCancel?.(job.id))}
							aria-label={isJobTerminal(job) ? 'Remove from list' : 'Cancel job'}
						>
							<span class="group-hover:hidden">
								{#if job.type === 'copy'}
									<Copy size={20} />
								{:else if job.type === 'move'}
									<FolderInput size={20} />
								{:else if job.type === 'delete'}
									<Trash2 size={20} />
								{:else}
									<Settings size={20} />
								{/if}
							</span>
							<span class="hidden group-hover:block">
								<X size={20} />
							</span>
						</button>

						<div class="flex min-w-0 flex-1 flex-col gap-1 py-0.5">
							<div class="flex items-center justify-between gap-2">
								<span
									class="overflow-hidden text-sm font-medium text-ellipsis whitespace-nowrap text-text-primary"
									title={job.sourcePath}
								>
									{getFileName(job.sourcePath)}
								</span>
								<span class="shrink-0 text-xs text-text-muted">
									{formatFileDate(job.createdAt)}
								</span>
							</div>
							<div
								class="overflow-hidden text-xs text-ellipsis whitespace-nowrap text-text-muted"
								title="{getFolderPath(job.sourcePath)}{job.destPath
									? ` → ${getFolderPath(job.destPath)}`
									: ''}"
							>
								{getFolderPath(job.sourcePath)}{#if job.destPath}<span class="mx-1">→</span
									>{getFolderPath(job.destPath)}{/if}
							</div>

							<!-- Progress bar with inline status on same row -->
							<div class="flex items-center gap-3">
								<div class="flex-1">
									<ProgressBar
										value={job.state === 'completed' ? 100 : job.progress}
										size="sm"
										variant={job.state === 'completed'
											? 'success'
											: job.state === 'failed'
												? 'danger'
												: 'default'}
									/>
								</div>
								<span
									class="shrink-0 text-[11px] {job.state === 'completed'
										? 'text-success'
										: ''} {job.state === 'failed' ? 'text-danger' : ''} {job.state === 'running'
										? 'text-accent'
										: 'text-text-muted'}"
								>
									{getStatusText(job)}
								</span>
							</div>
						</div>
					</div>
				</li>
			{/each}
		</ul>

		{#if jobs.length > maxDisplay}
			<div
				class="border-t border-border-secondary bg-surface-primary px-4 py-2 text-center text-xs text-text-muted"
			>
				+{jobs.length - maxDisplay} more jobs
			</div>
		{/if}
	</div>
{/if}
