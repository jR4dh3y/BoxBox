import assert from 'node:assert/strict';
import test from 'node:test';
import type { FileInfo, FileList } from '#lib/api/files.js';
import type { Job } from '#lib/api/jobs.js';
import {
	createTransferQueue,
	planTransfers,
	resolveTransferPlan,
	type ResolvedTransferPlan,
	type TransferSource
} from './transfers.js';

function file(name: string, path: string): FileInfo {
	return {
		name,
		path,
		size: 0,
		isDir: false,
		modTime: '',
		permissions: ''
	};
}

function fileList(items: FileInfo[], totalCount = items.length, page = 1): FileList {
	return { path: 'destination', items, totalCount, page, pageSize: 1000 };
}

function source(name: string, path = `source/${name}`): TransferSource {
	return { name, path, isDir: false };
}

function job(id: string, sourcePath: string, destPath: string): Job {
	return {
		id,
		type: 'copy',
		state: 'pending',
		progress: 0,
		sourcePath,
		destPath,
		createdAt: '2026-10-21T00:00:00.000Z'
	};
}

function resolvedPlan(transfers: ResolvedTransferPlan['transfers']): ResolvedTransferPlan {
	return { operation: 'copy', transfers, skipped: [] };
}

test('plans keep-both names without losing extensions or colliding with planned copies', async () => {
	const plan = await planTransfers(
		'copy',
		[source('report.txt'), source('.env'), source('archive.tar.gz')],
		'destination',
		{
			listDirectory: async () =>
				fileList([
					file('report.txt', 'destination/report.txt'),
					file('report (copy 1).txt', 'destination/report (copy 1).txt'),
					file('.env', 'destination/.env'),
					file('archive.tar.gz', 'destination/archive.tar.gz')
				])
		}
	);

	assert.deepEqual(
		plan.entries.map((entry) => entry.kind === 'conflict' && entry.keepBothPath),
		[
			'destination/report (copy 2).txt',
			'destination/.env (copy 1)',
			'destination/archive.tar (copy 1).gz'
		]
	);
});

test('planning rejects an empty source name before listing the destination', async () => {
	let listed = false;
	await assert.rejects(
		planTransfers('copy', [source('')], 'destination', {
			listDirectory: async () => {
				listed = true;
				return fileList([]);
			}
		}),
		/A transfer item has an invalid name or path/
	);
	assert.equal(listed, false);
});

test('planner reads hidden entries and every page, rejecting a changing destination', async () => {
	const requests: Array<{ page: number; includeHidden: boolean | undefined }> = [];
	const plan = await planTransfers('copy', [source('file-1001')], 'destination', {
		listDirectory: async (_path, options) => {
			const page = options?.page ?? 1;
			requests.push({ page, includeHidden: options?.includeHidden });
			return page === 1
				? fileList(
						Array.from({ length: 1000 }, (_, index) =>
							file(`file-${index}`, `destination/${index}`)
						),
						1001
					)
				: fileList([file('.hidden', 'destination/.hidden')], 1001, 2);
		}
	});

	assert.equal(plan.entries.length, 1);
	assert.deepEqual(requests, [
		{ page: 1, includeHidden: true },
		{ page: 2, includeHidden: true }
	]);

	await assert.rejects(
		planTransfers('copy', [source('next.txt')], 'destination', {
			listDirectory: async (_path, options) =>
				options?.page === 1 ? fileList([file('first', 'destination/first')], 2) : fileList([], 3, 2)
		}),
		/Destination contents changed/
	);
});

test('unresolved conflicts skip by default and keep-both uses the planned destination', async () => {
	const plan = await planTransfers('move', [source('item.txt')], 'destination', {
		listDirectory: async () => fileList([file('item.txt', 'destination/item.txt')])
	});
	const conflict = plan.entries[0];
	assert.equal(conflict.kind, 'conflict');
	if (conflict.kind !== 'conflict') throw new Error('expected a conflict');

	assert.deepEqual(resolveTransferPlan(plan).transfers, []);
	assert.deepEqual(
		resolveTransferPlan(plan, [{ entryId: conflict.id, resolution: 'keep-both' }]).transfers,
		[
			{
				operation: 'move',
				source: source('item.txt'),
				destinationPath: 'destination/item (copy 1).txt'
			}
		]
	);
});

test('transfer queue waits for each job request before starting the next', async () => {
	const calls: string[] = [];
	let releaseFirst: (() => void) | undefined;
	let startedFirst: (() => void) | undefined;
	const firstStarted = new Promise<void>((resolve) => {
		startedFirst = resolve;
	});
	const firstRequest = new Promise<void>((resolve) => {
		releaseFirst = resolve;
	});
	const queue = createTransferQueue(
		resolvedPlan([
			{
				operation: 'copy',
				source: source('one'),
				destinationPath: 'destination/one'
			},
			{
				operation: 'copy',
				source: source('two'),
				destinationPath: 'destination/two'
			}
		]),
		{
			dependencies: {
				createCopyJob: async (sourcePath, destinationPath) => {
					calls.push(sourcePath);
					if (calls.length === 1) {
						startedFirst?.();
						await firstRequest;
					}
					return job(`job-${calls.length}`, sourcePath, destinationPath);
				}
			}
		}
	);

	const resultPromise = queue.start();
	await firstStarted;
	assert.deepEqual(calls, ['source/one']);
	releaseFirst?.();
	const result = await resultPromise;

	assert.equal(result.state, 'submitted');
	assert.deepEqual(calls, ['source/one', 'source/two']);
});

test('a request race is returned as an error and stops the remaining batch', async () => {
	const calls: string[] = [];
	const requestError = new Error('destination appeared after preflight');
	const queue = createTransferQueue(
		resolvedPlan([
			{
				operation: 'copy',
				source: source('one'),
				destinationPath: 'destination/one'
			},
			{
				operation: 'copy',
				source: source('two'),
				destinationPath: 'destination/two'
			}
		]),
		{
			dependencies: {
				createCopyJob: async (sourcePath) => {
					calls.push(sourcePath);
					throw requestError;
				}
			}
		}
	);

	const result = await queue.start();
	assert.equal(result.state, 'failed');
	if (result.state !== 'failed') throw new Error('expected a failed batch');
	assert.equal(result.failure.error, requestError);
	assert.deepEqual(calls, ['source/one']);
});

test('an immediately failed job response stops the remaining batch', async () => {
	const calls: string[] = [];
	const queue = createTransferQueue(
		resolvedPlan([
			{
				operation: 'copy',
				source: source('one'),
				destinationPath: 'destination/one'
			},
			{
				operation: 'copy',
				source: source('two'),
				destinationPath: 'destination/two'
			}
		]),
		{
			dependencies: {
				createCopyJob: async (sourcePath, destinationPath) => {
					calls.push(sourcePath);
					return {
						...job('job-failed', sourcePath, destinationPath),
						state: 'failed',
						error: 'job queue is full'
					};
				}
			}
		}
	);

	const result = await queue.start();
	assert.equal(result.state, 'failed');
	if (result.state !== 'failed') throw new Error('expected a failed batch');
	assert.equal(
		result.failure.error instanceof Error ? result.failure.error.message : '',
		'job queue is full'
	);
	assert.deepEqual(calls, ['source/one']);
});

test('cancel stops later requests and cancels jobs already created by the batch', async () => {
	const calls: string[] = [];
	const cancelled: string[] = [];
	const queue = createTransferQueue(
		resolvedPlan([
			{
				operation: 'copy',
				source: source('one'),
				destinationPath: 'destination/one'
			},
			{
				operation: 'copy',
				source: source('two'),
				destinationPath: 'destination/two'
			}
		]),
		{
			onJobCreated: () => void queue.cancel(),
			dependencies: {
				createCopyJob: async (sourcePath, destinationPath) => {
					calls.push(sourcePath);
					return job('job-1', sourcePath, destinationPath);
				},
				cancelJob: async (jobId) => {
					cancelled.push(jobId);
					return { message: 'cancelled' };
				}
			}
		}
	);

	const result = await queue.start();
	assert.equal(result.state, 'cancelled');
	assert.deepEqual(calls, ['source/one']);
	assert.deepEqual(cancelled, ['job-1']);
});
