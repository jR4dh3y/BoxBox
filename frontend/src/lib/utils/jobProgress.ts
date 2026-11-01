import { isJobActive, type Job } from '#lib/api/jobs.js';
import { parseRemotePath } from '#lib/api/remotePaths.js';
import { formatFileSize } from './format';

/** What a transfer row can truthfully say about a job. */
export type JobProgress =
	| { kind: 'queued' }
	/** Running, but nothing measurable has happened yet: the total may be unknown. */
	| { kind: 'working' }
	| { kind: 'measured'; percent: number; totalBytes?: number }
	/** Every byte is written; the job is still verifying, deleting a moved source, or renaming. */
	| { kind: 'finishing' }
	| { kind: 'completed' }
	| { kind: 'failed'; message: string }
	| { kind: 'cancelled' };

export function jobProgress(job: Job): JobProgress {
	switch (job.state) {
		case 'pending':
			return { kind: 'queued' };
		case 'running':
			if (job.progress >= 100) return { kind: 'finishing' };
			// Zero with a known size is a real 0%; zero without one is not a measurement.
			if (job.progress <= 0 && !job.bytesTotal) return { kind: 'working' };
			return { kind: 'measured', percent: job.progress, totalBytes: job.bytesTotal };
		case 'completed':
			return { kind: 'completed' };
		case 'failed':
			return { kind: 'failed', message: job.error ?? 'Failed' };
		case 'cancelled':
			return { kind: 'cancelled' };
	}
}

const VERBS: Record<Job['type'], string> = {
	copy: 'Copying',
	move: 'Moving',
	delete: 'Deleting'
};

export function jobProgressLabel(job: Job, progress: JobProgress = jobProgress(job)): string {
	switch (progress.kind) {
		case 'queued':
			return 'Queued';
		case 'working':
			return `${VERBS[job.type]}…`;
		case 'measured':
			return progress.totalBytes
				? `${Math.floor(progress.percent)}% of ${formatFileSize(progress.totalBytes)}`
				: `${Math.floor(progress.percent)}%`;
		case 'finishing':
			return 'Finishing…';
		case 'completed':
			return 'Done';
		case 'failed':
			return progress.message;
		case 'cancelled':
			return 'Cancelled';
	}
}

function baseName(path: string): string {
	const reference = parseRemotePath(path);
	return reference.path.split('/').filter(Boolean).at(-1) ?? '';
}

/** The item's own name, without a server qualifier. */
export function jobItemName(job: Job): string {
	return baseName(job.sourcePath) || job.sourcePath;
}

/** The destination folder's name, or undefined for jobs without one. */
export function jobDestinationFolder(job: Job): string | undefined {
	if (!job.destPath) return undefined;
	const reference = parseRemotePath(job.destPath);
	const folder = reference.path.split('/').filter(Boolean).slice(0, -1).at(-1);
	return folder ?? (reference.kind === 'remote' ? 'server root' : undefined);
}

/** Weighted progress of the active jobs, or undefined unless every one knows its size. */
export function overallPercent(jobs: readonly Job[]): number | undefined {
	let total = 0;
	let done = 0;
	for (const job of jobs) {
		if (!job.bytesTotal) return undefined;
		total += job.bytesTotal;
		done += (job.bytesTotal * Math.min(100, job.progress)) / 100;
	}
	return total ? Math.floor((done / total) * 100) : undefined;
}

export function transferSummary(jobs: readonly Job[]): string {
	const active = jobs.filter(isJobActive);
	if (active.length) {
		const percent = overallPercent(active);
		const noun = `${active.length} transfer${active.length === 1 ? '' : 's'}`;
		return percent === undefined ? `${noun} in progress` : `${noun} · ${percent}%`;
	}
	const failed = jobs.filter((job) => job.state === 'failed').length;
	if (failed) return `${failed} transfer${failed === 1 ? '' : 's'} failed`;
	return `${jobs.length} transfer${jobs.length === 1 ? '' : 's'} finished`;
}

/** The bar's value: undefined is an indeterminate bar, null hides it for ended jobs. */
export function jobBarValue(progress: JobProgress): number | undefined | null {
	switch (progress.kind) {
		case 'queued':
			return 0;
		case 'working':
			return undefined;
		case 'measured':
			return progress.percent;
		case 'finishing':
		case 'completed':
			return 100;
		case 'failed':
		case 'cancelled':
			return null;
	}
}
