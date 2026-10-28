<script lang="ts">
	import { SvelteSet } from 'svelte/reactivity';
	import { cancelJob, isJobActive, type Job } from '#lib/api/jobs.js';
	import { jobsStore } from '#lib/stores/jobs.svelte.js';
	import { toastStore } from '#lib/stores/toast.svelte.js';
	import { Button, ProgressBar } from '#lib/components/ui/index.js';
	const startedAt = Date.now();
	const dismissed = new SvelteSet<string>();
	const cancelling = new SvelteSet<string>();
	const jobs = $derived(
		Array.from(jobsStore.jobs.values()).filter(
			(job) =>
				!dismissed.has(job.id) && (isJobActive(job) || Date.parse(job.createdAt) >= startedAt)
		)
	);
	async function cancel(job: Job) {
		cancelling.add(job.id);
		try {
			await cancelJob(job.id);
			await jobsStore.loadJobs();
		} catch (error) {
			toastStore.error(error instanceof Error ? error.message : 'Could not cancel transfer');
		} finally {
			cancelling.delete(job.id);
		}
	}
</script>

{#if jobs.length}
	<div
		class="pointer-events-auto max-h-64 space-y-3 overflow-y-auto rounded-lg border border-border-primary bg-surface-secondary p-3 text-xs shadow-lg"
		aria-label="File operations"
	>
		{#each jobs as job (job.id)}
			<div class="space-y-1">
				<div class="flex items-center gap-2">
					<span class="min-w-0 flex-1 truncate"
						>{job.type === 'copy' ? 'Copy' : job.type === 'move' ? 'Move' : 'Delete'}
						{job.sourcePath.split('/').at(-1)}</span
					>
					{#if isJobActive(job)}<Button
							size="sm"
							disabled={cancelling.has(job.id)}
							onclick={() => void cancel(job)}>Cancel</Button
						>{:else}<Button size="sm" onclick={() => dismissed.add(job.id)}>Dismiss</Button>{/if}
				</div>
				<p role="status" class={job.state === 'failed' ? 'text-danger' : 'text-text-secondary'}>
					{job.error ?? job.state}{#if isJobActive(job)}
						· {job.progress}%{/if}
				</p>
				{#if isJobActive(job)}<ProgressBar value={job.progress} size="sm" />{/if}
			</div>
		{/each}
	</div>
{/if}
