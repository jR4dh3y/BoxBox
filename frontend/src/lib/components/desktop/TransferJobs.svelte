<script lang="ts">
	import { SvelteSet } from 'svelte/reactivity';
	import { Copy, FolderInput, Trash2, X } from 'lucide-svelte';
	import { cancelJob, getJob, isJobActive, type Job } from '#lib/api/jobs.js';
	import { jobsStore } from '#lib/stores/jobs.svelte.js';
	import { toastStore } from '#lib/stores/toast.svelte.js';
	import { ProgressBar } from '#lib/components/ui/index.js';
	import {
		jobBarValue,
		jobDestinationFolder,
		jobItemName,
		jobProgress,
		jobProgressLabel,
		transferSummary
	} from '#lib/utils/jobProgress.js';

	const dismissed = new SvelteSet<string>();
	const cancelling = new SvelteSet<string>();
	const jobs = $derived(
		Array.from(jobsStore.jobs.values()).filter(
			(job) => !dismissed.has(job.id) && (isJobActive(job) || jobsStore.recent.has(job.id))
		)
	);
	const finished = $derived(jobs.filter((job) => !isJobActive(job)));
	const icons = { copy: Copy, move: FolderInput, delete: Trash2 };

	async function cancel(job: Job) {
		cancelling.add(job.id);
		try {
			await cancelJob(job.id);
			jobsStore.upsertJob(await getJob(job.id));
		} catch (error) {
			toastStore.error(error instanceof Error ? error.message : 'Could not cancel transfer');
		} finally {
			cancelling.delete(job.id);
		}
	}
</script>

{#if jobs.length}
	<section
		class="pointer-events-auto overflow-hidden rounded-lg border border-border-primary bg-surface-secondary text-xs shadow-2xl"
		aria-label="File operations"
	>
		<div
			class="flex h-9 items-center justify-between gap-2 border-b border-border-secondary bg-surface-primary pr-1.5 pl-3"
		>
			<span class="min-w-0 truncate text-[13px] font-medium text-text-primary"
				>{transferSummary(jobs)}</span
			>
			{#if finished.length}
				<button
					type="button"
					class="shrink-0 cursor-pointer rounded px-2 py-1 text-text-secondary transition-colors hover:bg-surface-elevated hover:text-text-primary"
					onclick={() => finished.forEach((job) => dismissed.add(job.id))}>Clear finished</button
				>
			{/if}
		</div>
		<ul class="m-0 max-h-64 list-none overflow-y-auto p-0">
			{#each jobs as job (job.id)}
				{@const progress = jobProgress(job)}
				{@const Icon = icons[job.type]}
				{@const active = isJobActive(job)}
				{@const folder = jobDestinationFolder(job)}
				{@const bar = jobBarValue(progress)}
				<li
					class="flex items-stretch gap-3 border-b border-border-secondary px-3 py-2 last:border-b-0"
				>
					<!-- The type icon doubles as cancel or dismiss, as in the old job monitor. -->
					<button
						type="button"
						class="group flex w-9 shrink-0 cursor-pointer items-center justify-center rounded bg-surface-elevated text-text-secondary transition-colors hover:bg-danger/20 hover:text-danger focus-visible:bg-danger/20 focus-visible:text-danger disabled:cursor-wait disabled:opacity-60"
						disabled={cancelling.has(job.id)}
						aria-label={active ? `Cancel ${jobItemName(job)}` : `Dismiss ${jobItemName(job)}`}
						title={active ? 'Cancel' : 'Dismiss'}
						onclick={() => (active ? void cancel(job) : dismissed.add(job.id))}
					>
						<span class="group-hover:hidden group-focus-visible:hidden"><Icon size={16} /></span>
						<span class="hidden group-hover:block group-focus-visible:block"><X size={16} /></span>
					</button>
					<div class="flex min-w-0 flex-1 flex-col gap-1">
						<div class="flex min-w-0 items-baseline gap-1.5">
							<span
								class="min-w-0 flex-1 truncate text-[13px] text-text-primary"
								title={job.sourcePath}>{jobItemName(job)}</span
							>
							{#if folder}<span
									class="max-w-[45%] min-w-0 truncate text-text-muted"
									title={job.destPath}>→ {folder}</span
								>{/if}
						</div>
						<div class="flex items-center gap-3">
							{#if bar !== null}
								<div class="flex-1">
									<ProgressBar
										size="sm"
										label="{jobItemName(job)} progress"
										value={bar}
										variant={progress.kind === 'completed' ? 'success' : 'default'}
									/>
								</div>
							{/if}
							<span
								role="status"
								class="{bar === null
									? 'min-w-0 flex-1'
									: 'max-w-[55%] shrink-0'} truncate text-[11px] {progress.kind === 'failed'
									? 'text-danger'
									: progress.kind === 'completed'
										? 'text-success'
										: 'text-text-muted'}"
								title={jobProgressLabel(job, progress)}>{jobProgressLabel(job, progress)}</span
							>
						</div>
					</div>
				</li>
			{/each}
		</ul>
	</section>
{/if}
