import { isLocalDesktop } from './desktop';
/**
 * Job API module for background job operations
 */

import { api } from './client';
import { array, optionalString, record, string } from './parse';
import {
	collectRemote,
	number,
	parseRemotePath,
	pathEndpoint,
	qualifyPath,
	remoteSource,
	sameSource,
	sourceEndpoint,
	type PathReference
} from './remotePaths';

/**
 * Job types for background operations
 */
export type JobType = 'copy' | 'move' | 'delete';

/**
 * Job states
 */
const JOB_STATES = ['pending', 'running', 'completed', 'failed', 'cancelled'] as const;
export type JobState = (typeof JOB_STATES)[number];

export function isJobState(value: unknown): value is JobState {
	return JOB_STATES.some((state) => state === value);
}

/**
 * Job progress pushed over the WebSocket
 */
export interface JobUpdate {
	jobId: string;
	state: JobState;
	progress: number;
	error?: string;
}

/**
 * Job information
 */
export interface Job {
	id: string;
	type: JobType;
	state: JobState;
	progress: number;
	/** Bytes the job copies, once known; servers before byte progress omit it. */
	bytesTotal?: number;
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

export function parseJob(value: unknown, source: PathReference): Job {
	const job = record(value, 'job');
	const type = job.type;
	if (type !== 'copy' && type !== 'move' && type !== 'delete') throw new Error('Invalid job type');
	if (!isJobState(job.state)) throw new Error('Invalid job state');
	const destination = optionalString(job.destPath, 'job destination');
	return {
		id: qualifyPath(source, string(job.id, 'job ID')),
		type,
		state: job.state,
		progress: number(job.progress, 'job progress'),
		...(job.bytesTotal != null ? { bytesTotal: number(job.bytesTotal, 'job size') } : {}),
		sourcePath: qualifyPath(source, string(job.sourcePath, 'job source')),
		...(destination ? { destPath: qualifyPath(source, destination) } : {}),
		...(job.error != null ? { error: optionalString(job.error, 'job error') } : {}),
		createdAt: string(job.createdAt, 'created time'),
		...(job.startedAt != null ? { startedAt: optionalString(job.startedAt, 'start time') } : {}),
		...(job.completedAt != null
			? { completedAt: optionalString(job.completedAt, 'completion time') }
			: {})
	};
}

export function parseJobs(value: unknown, source: PathReference): Job[] {
	return array(record(value, 'jobs response').jobs, 'jobs').map((job) => parseJob(job, source));
}

const LOCAL: PathReference = { kind: 'local', path: '' };

async function transferJobs(): Promise<Job[]> {
	if (!isLocalDesktop()) return [];
	try {
		return parseJobs(await api.get<unknown>('/desktop/transfers'), LOCAL);
	} catch {
		return [];
	}
}

function jobEndpoint(id: string): string {
	return id.startsWith('transfer-')
		? `/desktop/transfers/${encodeURIComponent(id)}`
		: pathEndpoint(id, '/jobs');
}

/**
 * List all jobs
 * GET /api/v1/jobs
 */
export async function listJobs(): Promise<JobListResponse> {
	const [local, remote, transfers] = await Promise.all([
		api.get<unknown>('/jobs').then((value) => parseJobs(value, LOCAL)),
		collectRemote(async (server) =>
			parseJobs(
				await api.get<unknown>(sourceEndpoint(remoteSource(server), '/jobs')),
				remoteSource(server)
			)
		),
		transferJobs()
	]);
	return { jobs: [...local, ...remote, ...transfers] };
}

/**
 * Get a specific job by ID
 * GET /api/v1/jobs/:id
 */
export async function getJob(jobId: string, signal?: AbortSignal): Promise<Job> {
	return parseJob(
		await api.get<unknown>(jobEndpoint(jobId), undefined, signal),
		parseRemotePath(jobId)
	);
}

/**
 * Create a new background job
 * POST /api/v1/jobs
 */
export async function createJob(request: CreateJobRequest): Promise<Job> {
	const source = parseRemotePath(request.sourcePath);
	const destination =
		request.destPath === undefined ? undefined : parseRemotePath(request.destPath);
	if (destination && !sameSource(source, destination)) {
		if (request.type === 'move')
			throw new Error('Moving between servers is unavailable. Copy the items instead.');
		if (request.type === 'delete') throw new Error('Delete cannot have a cross-source destination');
		return parseJob(await api.post<unknown>('/desktop/transfers', request), LOCAL);
	}
	return parseJob(
		await api.post<unknown>(sourceEndpoint(source, '/jobs'), {
			...request,
			sourcePath: source.path,
			...(destination ? { destPath: destination.path } : {})
		}),
		source
	);
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
	const value = record(await api.delete<unknown>(jobEndpoint(jobId)), 'cancel response');
	return { message: string(value.message, 'message') };
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
