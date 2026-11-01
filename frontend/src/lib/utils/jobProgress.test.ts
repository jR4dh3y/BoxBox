import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import type { Job } from '#lib/api/jobs.js';
import {
	jobBarValue,
	jobDestinationFolder,
	jobItemName,
	jobProgress,
	jobProgressLabel,
	overallPercent,
	transferSummary
} from './jobProgress';

const SERVER = '0f8fad5b-d9cb-469f-a165-70867728950e';
function job(overrides: Partial<Job>): Job {
	return {
		id: 'job',
		type: 'copy',
		state: 'running',
		progress: 0,
		sourcePath: 'media/movie.mkv',
		destPath: 'backup/films/movie.mkv',
		createdAt: '2026-10-28T00:00:00Z',
		...overrides
	};
}

describe('job progress', () => {
	test('a running job without a size or progress is indeterminate, not 0%', () => {
		const progress = jobProgress(job({}));
		assert.deepEqual(progress, { kind: 'working' });
		assert.equal(jobBarValue(progress), undefined);
		assert.equal(jobProgressLabel(job({})), 'Copying…');
	});

	test('a known size makes 0% a real measurement', () => {
		const running = job({ bytesTotal: 300 * 1024 * 1024 });
		assert.equal(jobBarValue(jobProgress(running)), 0);
		assert.equal(jobProgressLabel(running), '0% of 300 MB');
	});

	test('older servers without a size still show their percentage', () => {
		assert.equal(jobProgressLabel(job({ progress: 42 })), '42%');
	});

	test('every byte written but still running reads as finishing', () => {
		const finishing = job({ progress: 100, bytesTotal: 10 });
		assert.equal(jobProgressLabel(finishing), 'Finishing…');
		assert.equal(jobBarValue(jobProgress(finishing)), 100);
	});

	test('ended jobs say how they ended and failures hide the bar', () => {
		assert.equal(jobProgressLabel(job({ state: 'pending' })), 'Queued');
		assert.equal(jobProgressLabel(job({ state: 'completed', progress: 100 })), 'Done');
		const failed = job({ state: 'failed', error: 'Destination already exists' });
		assert.equal(jobProgressLabel(failed), 'Destination already exists');
		assert.equal(jobBarValue(jobProgress(failed)), null);
		assert.equal(jobBarValue(jobProgress(job({ state: 'cancelled' }))), null);
	});

	test('names never expose a remote server ID', () => {
		const remote = job({
			sourcePath: `@remote/${SERVER}/files/report.pdf`,
			destPath: `@remote/${SERVER}/report.pdf`
		});
		assert.equal(jobItemName(remote), 'report.pdf');
		assert.equal(jobDestinationFolder(remote), 'server root');
		assert.equal(jobDestinationFolder(job({})), 'films');
		assert.equal(jobDestinationFolder(job({ type: 'delete', destPath: undefined })), undefined);
	});

	test('overall progress is weighted by size and unknown when any size is', () => {
		const big = job({ progress: 50, bytesTotal: 300 });
		const small = job({ progress: 100, bytesTotal: 100 });
		assert.equal(overallPercent([big, small]), 62);
		assert.equal(overallPercent([big, job({ progress: 10 })]), undefined);
		assert.equal(transferSummary([big, small]), '2 transfers · 62%');
		assert.equal(transferSummary([job({})]), '1 transfer in progress');
		assert.equal(
			transferSummary([job({ state: 'failed' }), job({ state: 'completed' })]),
			'1 transfer failed'
		);
		assert.equal(transferSummary([job({ state: 'completed' })]), '1 transfer finished');
	});
});
