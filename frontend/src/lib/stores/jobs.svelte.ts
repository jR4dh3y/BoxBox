/**
 * Jobs store using Svelte 5 runes.
 */

import {
	listJobs,
	getJob,
	createJob,
	cancelJob,
	isJobActive,
	isJobTerminal,
	type Job,
	type JobState,
	type CreateJobRequest
} from '$lib/api/jobs';
import { CONFIG } from '$lib/config';
import { SvelteDate, SvelteMap } from 'svelte/reactivity';

export interface JobUpdate {
	jobId: string;
	state: JobState;
	progress: number;
	error?: string;
}

export interface JobsState {
	jobs: SvelteMap<string, Job>;
	isLoading: boolean;
	error: string | null;
}

const initialState: JobsState = {
	jobs: new SvelteMap(),
	isLoading: false,
	error: null
};

class JobsStore {
	private state = $state<JobsState>({
		jobs: new SvelteMap(),
		isLoading: false,
		error: null
	});

	get jobs(): SvelteMap<string, Job> {
		return this.state.jobs;
	}

	get isLoading(): boolean {
		return this.state.isLoading;
	}

	get error(): string | null {
		return this.state.error;
	}

	get jobsList(): Job[] {
		return Array.from(this.state.jobs.values()).sort(
			(a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
		);
	}

	get activeJobs(): Job[] {
		return this.jobsList.filter(isJobActive);
	}

	get activeJobsCount(): number {
		return this.activeJobs.length;
	}

	get hasActiveJobs(): boolean {
		return this.activeJobsCount > 0;
	}

	get completedJobs(): Job[] {
		return this.jobsList
			.filter((job) => job.state === 'completed')
			.sort((a, b) => {
				return (
					new Date(b.completedAt ?? b.createdAt).getTime() -
					new Date(a.completedAt ?? a.createdAt).getTime()
				);
			});
	}

	get failedJobs(): Job[] {
		return this.jobsList
			.filter((job) => job.state === 'failed')
			.sort((a, b) => {
				return (
					new Date(b.completedAt ?? b.createdAt).getTime() -
					new Date(a.completedAt ?? a.createdAt).getTime()
				);
			});
	}

	async loadJobs(): Promise<void> {
		this.state.isLoading = true;
		this.state.error = null;

		try {
			const response = await listJobs();
			const jobsMap = new SvelteMap<string, Job>();
			for (const job of response.jobs) {
				jobsMap.set(job.id, job);
			}
			this.state.jobs = jobsMap;
			this.state.isLoading = false;
		} catch (err) {
			this.state.isLoading = false;
			this.state.error = err instanceof Error ? err.message : 'Failed to load jobs';
		}
	}

	upsertJob(job: Job): void {
		const next = new SvelteMap(this.state.jobs);
		next.set(job.id, job);
		this.state.jobs = next;
	}

	updateFromWebSocket(jobUpdate: JobUpdate): void {
		const existingJob = this.state.jobs.get(jobUpdate.jobId);
		if (!existingJob) {
			return;
		}

		const updatedJob: Job = {
			...existingJob,
			state: jobUpdate.state,
			progress: jobUpdate.progress,
			error: jobUpdate.error
		};

		if (isJobTerminal(updatedJob) && !updatedJob.completedAt) {
			updatedJob.completedAt = new SvelteDate().toISOString();
		}

		const next = new SvelteMap(this.state.jobs);
		next.set(jobUpdate.jobId, updatedJob);
		this.state.jobs = next;
	}

	removeJob(jobId: string): void {
		const next = new SvelteMap(this.state.jobs);
		next.delete(jobId);
		this.state.jobs = next;
	}

	clearTerminalJobs(): void {
		const next = new SvelteMap<string, Job>();
		for (const [id, job] of this.state.jobs) {
			if (isJobActive(job)) {
				next.set(id, job);
			}
		}
		this.state.jobs = next;
	}

	getJobById(jobId: string): Job | undefined {
		return this.state.jobs.get(jobId);
	}

	getAllJobs(): Job[] {
		return Array.from(this.state.jobs.values());
	}

	getActiveJobs(): Job[] {
		return this.getAllJobs().filter(isJobActive);
	}

	clearError(): void {
		this.state.error = null;
	}

	reset(): void {
		this.state = {
			jobs: new SvelteMap(initialState.jobs),
			isLoading: initialState.isLoading,
			error: initialState.error
		};
	}
}

export const jobsStore = new JobsStore();

export const jobQueryKeys = {
	all: ['jobs'] as const,
	list: () => [...jobQueryKeys.all, 'list'] as const,
	detail: (id: string) => [...jobQueryKeys.all, 'detail', id] as const
};

export function jobsQueryOptions() {
	return {
		queryKey: jobQueryKeys.list(),
		queryFn: () => listJobs(),
		refetchInterval: CONFIG.query.jobsRefetchIntervalMs
	};
}

export function jobQueryOptions(jobId: string) {
	return {
		queryKey: jobQueryKeys.detail(jobId),
		queryFn: () => getJob(jobId),
		enabled: !!jobId
	};
}

export function createJobMutationOptions() {
	return {
		mutationFn: (request: CreateJobRequest) => createJob(request),
		onSuccess: (job: Job) => {
			jobsStore.upsertJob(job);
		}
	};
}

export function cancelJobMutationOptions() {
	return {
		mutationFn: (jobId: string) => cancelJob(jobId),
		onSuccess: (_: unknown, jobId: string) => {
			const job = jobsStore.getJobById(jobId);
			if (job) {
				jobsStore.upsertJob({
					...job,
					state: 'cancelled',
					completedAt: new SvelteDate().toISOString()
				});
			}
		}
	};
}

export { isJobActive, isJobTerminal } from '$lib/api/jobs';
