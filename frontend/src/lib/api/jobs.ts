/**
 * Job API module for background job operations
 * Requirements: 4.1, 4.4, 4.5
 */

import { api } from './client';

/**
 * Job types for background operations
 */
export type JobType = 'copy' | 'move' | 'delete';

/**
 * Job states
 */
export type JobState = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

/**
 * Job information
 */
export interface Job {
	id: string;
	type: JobType;
	state: JobState;
	progress: number;
	sourcePath: string;
	destPath?: string;
	error?: string;
	createdAt: string;
	startedAt?: string;
	completedAt?: string;
}

/**
 * Job list response
 */
export interface JobListResponse {
	jobs: Job[];
}

/**
 * Create job request
 */
export interface CreateJobRequest {
	type: JobType;
	sourcePath: string;
	destPath?: string;
}

/**
 * Success message response
 */
interface MessageResponse {
	message: string;
}

/**
 * List all jobs
 * GET /api/v1/jobs
 */
export async function listJobs(): Promise<JobListResponse> {
	return api.get<JobListResponse>('/jobs');
}

/**
 * Get a specific job by ID
 * GET /api/v1/jobs/:id
 */
export async function getJob(jobId: string): Promise<Job> {
	return api.get<Job>(`/jobs/${jobId}`);
}

/**
 * Create a new background job
 * POST /api/v1/jobs
 */
export async function createJob(request: CreateJobRequest): Promise<Job> {
	return api.post<Job>('/jobs', request);
}

/**
 * Create a copy job
 */
export async function createCopyJob(sourcePath: string, destPath: string): Promise<Job> {
	return createJob({ type: 'copy', sourcePath, destPath });
}

/**
 * Create a move job
 */
export async function createMoveJob(sourcePath: string, destPath: string): Promise<Job> {
	return createJob({ type: 'move', sourcePath, destPath });
}

/**
 * Create a delete job
 */
export async function createDeleteJob(sourcePath: string): Promise<Job> {
	return createJob({ type: 'delete', sourcePath });
}

/**
 * Cancel a running job
 * DELETE /api/v1/jobs/:id
 */
export async function cancelJob(jobId: string): Promise<MessageResponse> {
	return api.delete<MessageResponse>(`/jobs/${jobId}`);
}

/**
 * Check if a job is in a terminal state
 */
export function isJobTerminal(job: Job): boolean {
	return job.state === 'completed' || job.state === 'failed' || job.state === 'cancelled';
}

/**
 * Check if a job is active (pending or running)
 */
export function isJobActive(job: Job): boolean {
	return job.state === 'pending' || job.state === 'running';
}
