/**
 * Jobs store for tracking active background jobs
 * Requirements: 4.4
 */

import { writable, derived } from 'svelte/store';
import { listJobs, isJobActive, isJobTerminal, type Job, type JobState } from '$lib/api/jobs';

/**
 * Job update from WebSocket
 */
export interface JobUpdate {
	jobId: string;
	state: JobState;
	progress: number;
	error?: string;
}

/**
 * Jobs state
 */
export interface JobsState {
	jobs: Map<string, Job>;
	isLoading: boolean;
	error: string | null;
}

/**
 * Initial jobs state
 */
const initialState: JobsState = {
	jobs: new Map(),
	isLoading: false,
	error: null
};

/**
 * Create the jobs store
 */
function createJobsStore() {
	const { subscribe, set, update } = writable<JobsState>(initialState);

	/**
	 * Load all jobs from the API
	 */
	async function loadJobs(): Promise<void> {
		update((state) => ({ ...state, isLoading: true, error: null }));

		try {
			const response = await listJobs();
			const jobsMap = new Map<string, Job>();
			for (const job of response.jobs) {
				jobsMap.set(job.id, job);
			}
			update((state) => ({
				...state,
				jobs: jobsMap,
				isLoading: false
			}));
		} catch (err) {
			const message = err instanceof Error ? err.message : 'Failed to load jobs';
			update((state) => ({
				...state,
				isLoading: false,
				error: message
			}));
		}
	}

	/**
	 * Add or update a job in the store
	 */
	function upsertJob(job: Job): void {
		update((state) => {
			const newJobs = new Map(state.jobs);
			newJobs.set(job.id, job);
			return { ...state, jobs: newJobs };
		});
	}

	/**
	 * Update a job from a WebSocket update
	 */
	function updateFromWebSocket(jobUpdate: JobUpdate): void {
		update((state) => {
			const existingJob = state.jobs.get(jobUpdate.jobId);
			if (!existingJob) return state;

			const updatedJob: Job = {
				...existingJob,
				state: jobUpdate.state,
				progress: jobUpdate.progress,
				error: jobUpdate.error
			};

			// Set completedAt if job is now terminal
			if (isJobTerminal(updatedJob) && !updatedJob.completedAt) {
				updatedJob.completedAt = new Date().toISOString();
			}

			const newJobs = new Map(state.jobs);
			newJobs.set(jobUpdate.jobId, updatedJob);
			return { ...state, jobs: newJobs };
		});
	}

	/**
	 * Reset store to initial state
	 */
	function reset(): void {
		set(initialState);
	}

	return {
		subscribe,
		loadJobs,
		upsertJob,
		updateFromWebSocket,
		reset
	};
}

/**
 * Jobs store singleton
 */
export const jobsStore = createJobsStore();

/**
 * Derived store for active jobs only
 */
export const activeJobs = derived(jobsStore, ($jobs) =>
	Array.from($jobs.jobs.values())
		.filter(isJobActive)
		.sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime())
);
