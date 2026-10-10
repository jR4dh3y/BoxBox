import assert from 'node:assert/strict';
import { afterEach, describe, test } from 'node:test';
import { QueryClient } from '@tanstack/svelte-query';
import type { Job, JobState } from '$lib/api/jobs';
import { fileQueryKeys } from './files';
import { jobsStore } from './jobs.svelte';
import { refreshFilesWhenJobsFinish } from './refreshFilesOnJobs';

function job(state: JobState): Job {
	return {
		id: 'job-1',
		type: 'delete',
		state,
		progress: 0,
		sourcePath: 'media/old',
		createdAt: '2026-01-01T00:00:00Z'
	};
}

const listKey = fileQueryKeys.list('media', { pageSize: 50, sortBy: 'name', sortDir: 'asc' });
const statsKey = [...fileQueryKeys.roots(), 'stats'];

afterEach(() => {
	jobsStore.onFinished = null;
	jobsStore.reset();
});

describe('jobsStore.onFinished', () => {
	test('fires once when a running job ends, not for progress', () => {
		let calls = 0;
		jobsStore.onFinished = () => calls++;
		jobsStore.upsertJob(job('running'));

		jobsStore.updateFromWebSocket({ jobId: 'job-1', state: 'running', progress: 40 });
		assert.equal(calls, 0);

		jobsStore.updateFromWebSocket({ jobId: 'job-1', state: 'completed', progress: 100 });
		jobsStore.updateFromWebSocket({ jobId: 'job-1', state: 'completed', progress: 100 });
		assert.equal(calls, 1);
	});

	test('fires for failed and cancelled jobs, which may have changed files', () => {
		let calls = 0;
		jobsStore.onFinished = () => calls++;
		for (const state of ['failed', 'cancelled'] as const) {
			jobsStore.upsertJob(job('pending'));
			jobsStore.updateFromWebSocket({ jobId: 'job-1', state, progress: 10 });
		}
		assert.equal(calls, 2);
	});

	test('ignores updates for unknown jobs', () => {
		let calls = 0;
		jobsStore.onFinished = () => calls++;
		jobsStore.updateFromWebSocket({ jobId: 'other', state: 'completed', progress: 100 });
		assert.equal(calls, 0);
	});
});

describe('refreshFilesWhenJobsFinish', () => {
	test('marks cached listings and drive stats stale when a job ends, not before', () => {
		const queryClient = new QueryClient();
		queryClient.setQueryData(listKey, { pages: [], pageParams: [] });
		queryClient.setQueryData(statsKey, { drives: [] });
		const stop = refreshFilesWhenJobsFinish(queryClient);
		jobsStore.upsertJob(job('running'));

		jobsStore.updateFromWebSocket({ jobId: 'job-1', state: 'running', progress: 50 });
		assert.equal(queryClient.getQueryState(listKey)?.isInvalidated, false);

		jobsStore.updateFromWebSocket({ jobId: 'job-1', state: 'completed', progress: 100 });
		assert.equal(queryClient.getQueryState(listKey)?.isInvalidated, true);
		assert.equal(queryClient.getQueryState(statsKey)?.isInvalidated, true);
		stop();
	});

	test('leaves the cache alone after it is stopped', () => {
		const queryClient = new QueryClient();
		queryClient.setQueryData(listKey, { pages: [], pageParams: [] });
		refreshFilesWhenJobsFinish(queryClient)();
		jobsStore.upsertJob(job('running'));
		jobsStore.updateFromWebSocket({ jobId: 'job-1', state: 'completed', progress: 100 });
		assert.equal(queryClient.getQueryState(listKey)?.isInvalidated, false);
	});
});
