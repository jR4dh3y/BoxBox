import assert from 'node:assert/strict';
import { afterEach, beforeEach, describe, test } from 'node:test';
import {
	createCopyJob,
	createMoveJob,
	createDeleteJob,
	type Job,
	type JobState
} from '#lib/api/jobs.js';
import { jobsStore } from './jobs.svelte';

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

const realFetch = globalThis.fetch;
const realWindow = globalThis.window;

/** The server's job list, as `loadJobs` will see it. */
function serveJobs(...jobs: Job[]) {
	Object.assign(globalThis, {
		// The API client builds its URLs from the page origin.
		window: { location: { origin: 'http://localhost' } },
		fetch: async () => Response.json({ jobs })
	});
}

/** Hold a captured HTTP snapshot until after live updates have arrived. */
function holdJobs(...jobs: Job[]) {
	let release!: (response: Response) => void;
	const response = new Promise<Response>((resolve) => (release = resolve));
	let started!: () => void;
	const requested = new Promise<void>((resolve) => (started = resolve));
	serveJobs();
	Object.assign(globalThis, {
		fetch: async () => {
			started();
			return response;
		}
	});
	return { requested, respond: () => release(Response.json({ jobs })) };
}

beforeEach(() => serveJobs());

afterEach(() => {
	Object.assign(globalThis, { fetch: realFetch, window: realWindow });
	jobsStore.onFinished = undefined;
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
			jobsStore.upsertJob({ ...job('pending'), id: state });
			jobsStore.updateFromWebSocket({ jobId: state, state, progress: 10 });
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

describe('delayed job creation during a reconnect reload', () => {
	for (const type of ['copy', 'move', 'delete'] as const) {
		for (const creationFirst of [true, false]) {
			test(`${type}: keeps completion when ${creationFirst ? 'creation' : 'listing'} arrives first`, async () => {
				const pending: Job = { ...job('pending'), type };
				const completed: Job = {
					...pending,
					state: 'completed',
					progress: 100,
					completedAt: '2026-01-01T00:01:00Z'
				};
				let releaseCreation!: () => void;
				let releaseListing!: () => void;
				const creationGate = new Promise<void>((resolve) => (releaseCreation = resolve));
				const listingGate = new Promise<void>((resolve) => (releaseListing = resolve));
				let creationStarted!: () => void;
				let listingStarted!: () => void;
				const creating = new Promise<void>((resolve) => (creationStarted = resolve));
				const listing = new Promise<void>((resolve) => (listingStarted = resolve));
				const server = Bun.serve({
					port: 0,
					async fetch(request) {
						if (request.method === 'POST') {
							creationStarted();
							await creationGate;
							return Response.json(pending);
						}
						listingStarted();
						await listingGate;
						return Response.json({ jobs: [completed] });
					}
				});
				Object.assign(globalThis, {
					window: { location: { origin: server.url.origin } },
					fetch: realFetch
				});

				let calls = 0;
				jobsStore.onFinished = () => {
					calls++;
				};
				const create = {
					copy: () => createCopyJob(pending.sourcePath, 'media/dest'),
					move: () => createMoveJob(pending.sourcePath, 'media/dest'),
					delete: () => createDeleteJob(pending.sourcePath)
				}[type];
				const creation = create().then((response) => jobsStore.upsertJob(response));
				let loading: Promise<void> | undefined;
				try {
					await creating;
					loading = jobsStore.loadJobs();
					await listing;
					// The completion arrives before the POST tells us the new job's ID.
					jobsStore.updateFromWebSocket({ jobId: pending.id, state: 'completed', progress: 100 });
					assert.equal(jobsStore.jobs.size, 0);
					if (creationFirst) {
						releaseCreation();
						await creation;
						releaseListing();
						await loading;
					} else {
						releaseListing();
						await loading;
						releaseCreation();
						await creation;
					}
					assert.deepEqual(jobsStore.jobs.get(pending.id), completed);
					assert.equal(jobsStore.active.length, 0);
					assert.equal(calls, 1);
					jobsStore.upsertJob(pending);
					await jobsStore.loadJobs();
					assert.equal(calls, 1, 'repeated observations must not notify twice');
				} finally {
					releaseCreation();
					releaseListing();
					await Promise.allSettled([creation, ...(loading ? [loading] : [])]);
					server.stop(true);
				}
			});
		}
	}
});

describe('jobsStore.loadJobs', () => {
	for (const state of ['completed', 'failed', 'cancelled'] as const) {
		test(`preserves a live ${state} update over an older running snapshot`, async () => {
			let calls = 0;
			jobsStore.onFinished = () => calls++;
			jobsStore.upsertJob(job('running'));
			const held = holdJobs(job('running'));
			const loading = jobsStore.loadJobs();
			await held.requested;

			jobsStore.updateFromWebSocket({ jobId: 'job-1', state, progress: 100 });
			const completed = jobsStore.jobs.get('job-1');
			held.respond();
			await loading;

			assert.equal(jobsStore.jobs.get('job-1'), completed);
			assert.equal(jobsStore.active.length, 0);
			assert.equal(calls, 1, 'completion must not be reported twice');
			assert.equal(jobsStore.isLoading, false);
		});
	}

	test('a terminal snapshot does not replace a socket completion already notified', async () => {
		let calls = 0;
		jobsStore.onFinished = () => calls++;
		jobsStore.upsertJob(job('running'));
		const held = holdJobs({ ...job('completed'), progress: 100 });
		const loading = jobsStore.loadJobs();
		await held.requested;
		jobsStore.updateFromWebSocket({ jobId: 'job-1', state: 'completed', progress: 100 });
		const live = jobsStore.jobs.get('job-1');
		held.respond();
		await loading;
		jobsStore.upsertJob(job('pending'));
		assert.equal(jobsStore.jobs.get('job-1'), live);
		assert.equal(calls, 1);
	});

	test('preserves live progress and jobs created while the list request waits', async () => {
		jobsStore.upsertJob(job('running'));
		const held = holdJobs(job('running'), { ...job('completed'), id: 'server-job' });
		const loading = jobsStore.loadJobs();
		await held.requested;
		jobsStore.updateFromWebSocket({ jobId: 'job-1', state: 'running', progress: 75 });
		jobsStore.upsertJob({ ...job('pending'), id: 'new-job' });
		held.respond();
		await loading;

		assert.equal(jobsStore.jobs.get('job-1')?.progress, 75);
		assert.equal(jobsStore.jobs.get('new-job')?.state, 'pending');
		assert.equal(jobsStore.jobs.get('server-job')?.state, 'completed');
	});

	test('preserves an updated job even when the HTTP snapshot omits it', async () => {
		jobsStore.upsertJob(job('running'));
		const held = holdJobs();
		const loading = jobsStore.loadJobs();
		await held.requested;
		jobsStore.updateFromWebSocket({ jobId: 'job-1', state: 'completed', progress: 100 });
		held.respond();
		await loading;

		assert.equal(jobsStore.jobs.get('job-1')?.state, 'completed');
	});

	test('a superseded request cannot overwrite the newer list', async () => {
		const held = holdJobs(job('running'));
		const loading = jobsStore.loadJobs();
		await held.requested;
		serveJobs(job('completed'));
		await jobsStore.loadJobs();
		held.respond();
		await loading;

		assert.equal(jobsStore.jobs.get('job-1')?.state, 'completed');
	});

	test('reset invalidates a pending request without disturbing a new load', async () => {
		const old = holdJobs(job('running'));
		const oldLoading = jobsStore.loadJobs();
		await old.requested;
		jobsStore.reset();
		const next = holdJobs({ ...job('pending'), id: 'new-session-job' });
		const nextLoading = jobsStore.loadJobs();
		await next.requested;
		old.respond();
		await oldLoading;
		assert.equal(jobsStore.jobs.size, 0);
		assert.equal(jobsStore.isLoading, true, 'old request must not finish the new load');
		next.respond();
		await nextLoading;
		assert.deepEqual([...jobsStore.jobs.keys()], ['new-session-job']);
		assert.equal(jobsStore.isLoading, false);
	});

	test('fires when a job we saw running has ended on the server, as after a reconnect', async () => {
		let calls = 0;
		jobsStore.onFinished = () => calls++;
		jobsStore.upsertJob(job('running'));

		serveJobs(job('completed'));
		await jobsStore.loadJobs();

		assert.equal(calls, 1);
		assert.equal(jobsStore.jobs.get('job-1')?.state, 'completed');
	});

	test('stays quiet when the job is still running, was already finished, or is new', async () => {
		let calls = 0;
		jobsStore.upsertJob(job('running'));
		jobsStore.upsertJob({ ...job('completed'), id: 'job-2' });
		jobsStore.onFinished = () => calls++;

		serveJobs(
			job('running'),
			{ ...job('completed'), id: 'job-2' },
			{ ...job('completed'), id: 'job-3' }
		);
		await jobsStore.loadJobs();

		assert.equal(calls, 0);
	});

	test('a late pending creation reply cannot replace a running snapshot', async () => {
		const running = { ...job('running'), progress: 65 };
		const held = holdJobs(running);
		const loading = jobsStore.loadJobs();
		await held.requested;
		jobsStore.upsertJob(job('pending'));
		held.respond();
		await loading;
		jobsStore.upsertJob(job('pending'));
		assert.deepEqual(jobsStore.jobs.get('job-1'), running);
	});

	for (const state of ['completed', 'failed', 'cancelled'] as const) {
		test(`a delayed creation cannot regress a ${state} snapshot`, async () => {
			let calls = 0;
			jobsStore.onFinished = () => calls++;
			const terminal = job(state);
			if (state === 'failed') terminal.error = 'Disk full';
			serveJobs(terminal);
			await jobsStore.loadJobs();
			assert.equal(calls, 0, 'historical jobs from the initial load do not notify');
			jobsStore.upsertJob(job('pending'));
			jobsStore.updateFromWebSocket({ jobId: 'job-1', state: 'running', progress: 10 });
			jobsStore.upsertJob(job('pending'));
			assert.deepEqual(jobsStore.jobs.get('job-1'), terminal);
			assert.equal(jobsStore.active.length, 0);
			assert.equal(calls, 1, 'a just-created job already finished on the server notifies once');
		});
	}

	test('marks every completion in a reload batch but invalidates only once', async () => {
		let calls = 0;
		jobsStore.onFinished = () => calls++;
		jobsStore.upsertJob(job('running'));
		jobsStore.upsertJob({ ...job('running'), id: 'job-2' });
		serveJobs(job('completed'), { ...job('completed'), id: 'job-2' });
		await jobsStore.loadJobs();
		assert.equal(calls, 1);
		jobsStore.upsertJob(job('pending'));
		jobsStore.upsertJob({ ...job('pending'), id: 'job-2' });
		assert.equal(calls, 1);
	});

	test('the first load, with nothing known yet, fires nothing', async () => {
		let calls = 0;
		jobsStore.onFinished = () => calls++;
		serveJobs(job('completed'));
		await jobsStore.loadJobs();
		assert.equal(calls, 0);
	});
});

describe('desktop polling lifecycle', () => {
	test('a slow load finishes before another poll; socket completion stays responsive', async () => {
		let calls = 0;
		jobsStore.onFinished = () => calls++;
		jobsStore.upsertJob(job('running'));
		jobsStore.upsertJob({ ...job('running'), id: 'local-socket-job' });
		const held = holdJobs({ ...job('running'), progress: 50 });
		const loading = jobsStore.loadJobs();
		await held.requested;
		const disconnect = jobsStore.connectPolling();
		try {
			await new Promise((resolve) => setTimeout(resolve, 2200));
			jobsStore.updateFromWebSocket({
				jobId: 'local-socket-job',
				state: 'completed',
				progress: 100
			});
			assert.equal(calls, 1, 'local socket publishes while remote listing is pending');
			held.respond();
			await loading;
			assert.equal(jobsStore.jobs.get('job-1')?.progress, 50);
			serveJobs({ ...job('completed'), progress: 100 });
			await new Promise((resolve) => setTimeout(resolve, 2100));
			assert.equal(jobsStore.jobs.get('job-1')?.state, 'completed');
			assert.equal(calls, 2);
			await jobsStore.loadJobs();
			assert.equal(calls, 2, 'terminal reload does not repeat invalidation');
		} finally {
			held.respond();
			disconnect();
			await loading;
		}
	});

	test('disconnect invalidates a held load and never schedules another poll', async () => {
		const held = holdJobs(job('running'));
		const loading = jobsStore.loadJobs();
		await held.requested;
		const disconnect = jobsStore.connectPolling();
		disconnect();
		held.respond();
		await loading;
		let requests = 0;
		Object.assign(globalThis, {
			fetch: async () => {
				requests++;
				return Response.json({ jobs: [] });
			}
		});
		await new Promise((resolve) => setTimeout(resolve, 2100));
		assert.equal(requests, 0);
		assert.equal(jobsStore.jobs.size, 0);
		assert.equal(jobsStore.isLoading, false);
	});
});
