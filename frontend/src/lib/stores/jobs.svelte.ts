/**
 * Background jobs the current user has started
 */

import { SvelteMap } from 'svelte/reactivity';
import { listJobs, isJobActive, isJobTerminal, type Job, type JobUpdate } from '#lib/api/jobs.js';

function newestFirst(left: Job, right: Job): number {
	return new Date(right.createdAt).getTime() - new Date(left.createdAt).getTime();
}

/** Delivery order is not state order: jobs cannot restart or move backwards. */
function mergeJob(current: Job | undefined, incoming: Job): Job {
	if (!current) return incoming;
	if (isJobTerminal(current)) return current;
	if (isJobTerminal(incoming)) return incoming;
	if (current.state === 'running' && incoming.state === 'pending') return current;
	if (current.state === incoming.state && current.progress > incoming.progress) return current;
	return incoming;
}

class JobsStore {
	jobs = new SvelteMap<string, Job>();
	/** Called when a pending or running job ends, however it ends: it may have changed files. */
	onFinished?: () => void;
	isLoading = $state(false);
	error = $state<string | null>(null);
	/** Invalidates overlapping loads and responses from a previous session. */
	private loadVersion = 0;
	private lastLoadAt = 0;
	/** Terminal objects are retained by mergeJob, so notifications stay once-only without retaining IDs. */
	private notifiedFinished = new WeakSet<Job>();

	/** Pending and running jobs, newest first */
	active = $derived(Array.from(this.jobs.values()).filter(isJobActive).sort(newestFirst));

	async loadJobs(): Promise<void> {
		const version = ++this.loadVersion;
		this.lastLoadAt = Date.now();
		const atRequest = new Map(this.jobs);
		this.isLoading = true;
		this.error = null;
		try {
			const response = await listJobs();
			if (version !== this.loadVersion) return;

			// Identity identifies writes during the request, not which server state is newer.
			// Merge by lifecycle too: a delayed creation reply must not undo server completion.
			const liveUpdates = Array.from(this.jobs).filter(([id, job]) => job !== atRequest.get(id));
			const wasActive = this.active;
			this.jobs.clear();
			for (const job of response.jobs) this.jobs.set(job.id, mergeJob(atRequest.get(job.id), job));
			for (const [id, job] of liveUpdates)
				this.jobs.set(id, mergeJob(job, this.jobs.get(id) ?? job));
			// Mark every completion, then invalidate once for the batch.
			if (wasActive.filter((job) => this.markFinished(this.jobs.get(job.id))).length)
				this.onFinished?.();
		} catch (err) {
			if (version === this.loadVersion) {
				this.error = err instanceof Error ? err.message : 'Failed to load jobs';
			}
		} finally {
			if (version === this.loadVersion) this.isLoading = false;
		}
	}

	/** Active transfers need frequent updates; idle reconciliation discovers jobs from other clients. */
	connectPolling(): () => void {
		const timer = setInterval(() => {
			if (typeof document !== 'undefined' && document.hidden) return;
			if (!this.isLoading && (this.active.length || Date.now() - this.lastLoadAt >= 30_000))
				void this.loadJobs();
		}, 2000);
		return () => {
			clearInterval(timer);
			this.reset();
		};
	}

	upsertJob(job: Job): void {
		const merged = mergeJob(this.jobs.get(job.id), job);
		this.jobs.set(job.id, merged);
		// A newly created job may already be terminal in a reload received before its POST reply.
		if (this.markFinished(merged)) this.onFinished?.();
	}

	private markFinished(job: Job | undefined): boolean {
		if (!job || !isJobTerminal(job) || this.notifiedFinished.has(job)) return false;
		this.notifiedFinished.add(job);
		return true;
	}

	updateFromWebSocket(update: JobUpdate): void {
		const existing = this.jobs.get(update.jobId);
		if (!existing || isJobTerminal(existing)) return;

		const job = mergeJob(existing, {
			...existing,
			state: update.state,
			progress: update.progress,
			error: update.error
		});
		if (isJobTerminal(job) && !job.completedAt) {
			job.completedAt = new Date().toISOString();
		}
		this.jobs.set(update.jobId, job);
		if (this.markFinished(job)) this.onFinished?.();
	}

	reset(): void {
		this.loadVersion++;
		this.lastLoadAt = 0;
		this.notifiedFinished = new WeakSet();
		this.jobs.clear();
		this.isLoading = false;
		this.error = null;
	}
}

export const jobsStore = new JobsStore();
