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

	let { jobs = [], onCancel, onRemove, onClearCompleted, showCompleted = true, maxDisplay = 10 }: Props = $props();

	const filteredJobs = $derived(
		(showCompleted ? jobs : jobs.filter((j) => !isJobTerminal(j))).slice(0, maxDisplay)
	);

	const activeCount = $derived(jobs.filter(isJobActive).length);
	const completedCount = $derived(jobs.filter(isJobTerminal).length);

	function getStatusText(job: Job): string {
		switch (job.state) {
			case 'pending': return 'Waiting...';
			case 'running': return `${formatPercentage(job.progress, 0, false)}`;
			case 'completed': return 'Completed';
			case 'failed': return job.error || 'Failed';
			case 'cancelled': return 'Cancelled';
			default: return '';
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
	<div class="bg-surface-secondary border border-border-primary rounded-lg overflow-hidden">
		<div class="px-4 py-3 bg-surface-primary border-b border-border-secondary flex items-center justify-between">
			<h3 class="m-0 text-sm font-semibold text-text-primary flex items-center gap-2">
				Background Jobs
				{#if activeCount > 0}
					<Badge variant="info">{activeCount} active</Badge>
				{/if}
			</h3>
			{#if completedCount > 0 && onClearCompleted}
				<Button variant="ghost" size="sm" onclick={onClearCompleted}>
					Clear done
				</Button>
			{/if}
		</div>

		<ul class="list-none m-0 p-0 max-h-[400px] overflow-y-auto" role="list">
			{#each filteredJobs as job (job.id)}
				<li class="px-4 py-3 border-b border-border-secondary last:border-b-0 transition-all hover:bg-surface-tertiary">
					<div class="flex items-stretch gap-3">
						<!-- Icon that transforms to X on hover - stretches to match content -->
						<button
							type="button"
							class="group shrink-0 w-16 flex items-center justify-center rounded bg-surface-elevated text-text-secondary border-none cursor-pointer transition-all hover:bg-danger/20 hover:text-danger"
							onclick={() => isJobTerminal(job) ? onRemove?.(job.id) : onCancel?.(job.id)}
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
						
						<div class="flex-1 min-w-0 flex flex-col gap-1 py-0.5">
							<div class="flex items-center justify-between gap-2">
								<span class="text-sm font-medium text-text-primary overflow-hidden text-ellipsis whitespace-nowrap" title={job.sourcePath}>
									{getFileName(job.sourcePath)}
								</span>
								<span class="text-xs text-text-muted shrink-0">
									{formatFileDate(job.createdAt)}
								</span>
							</div>
							<div class="text-xs text-text-muted overflow-hidden text-ellipsis whitespace-nowrap" title="{getFolderPath(job.sourcePath)}{job.destPath ? ` → ${getFolderPath(job.destPath)}` : ''}">
								{getFolderPath(job.sourcePath)}{#if job.destPath}<span class="mx-1">→</span>{getFolderPath(job.destPath)}{/if}
							</div>
							
							<!-- Progress bar with inline status on same row -->
							<div class="flex items-center gap-3">
								<div class="flex-1">
									<ProgressBar 
										value={job.state === 'completed' ? 100 : job.progress} 
										size="sm" 
										variant={job.state === 'completed' ? 'success' : job.state === 'failed' ? 'danger' : 'default'}
									/>
								</div>
								<span class="text-[11px] shrink-0 {job.state === 'completed' ? 'text-success' : ''} {job.state === 'failed' ? 'text-danger' : ''} {job.state === 'running' ? 'text-accent' : 'text-text-muted'}">
									{getStatusText(job)}
								</span>
							</div>
						</div>
					</div>
				</li>
			{/each}
		</ul>

		{#if jobs.length > maxDisplay}
			<div class="px-4 py-2 text-center text-xs text-text-muted bg-surface-primary border-t border-border-secondary">
				+{jobs.length - maxDisplay} more jobs
			</div>
		{/if}
	</div>
{/if}
