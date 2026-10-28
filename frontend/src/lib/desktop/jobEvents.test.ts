import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import { connectDesktopJobs } from './jobEvents';
import { jobsStore } from '#lib/stores/jobs.svelte.js';
import { invalidateRemoteServers } from '#lib/api/remotePaths.js';
const originalWindow = globalThis.window;
const encoder = new TextEncoder();
const job = (id = 'proof', state = 'running', progress = 5) => ({
	id,
	type: 'copy',
	state,
	progress,
	sourcePath: 'media/a',
	destPath: 'media/b',
	createdAt: '2026-10-28T00:00:00Z'
});
const settle = () => new Promise((resolve) => setTimeout(resolve, 40));
afterEach(() => {
	Object.assign(globalThis, { window: originalWindow });
	jobsStore.reset();
	jobsStore.onFinished = undefined;
});

test('desktop consumes real HTTP events without list polling, qualifies remote jobs and cancels streams', async () => {
	const writers = new Map<string, ReadableStreamDefaultController<Uint8Array>>();
	const counts = new Map<string, number>();
	let cancels = 0;
	const server = Bun.serve({
		port: 0,
		fetch(request) {
			const path = new URL(request.url).pathname;
			counts.set(path, (counts.get(path) ?? 0) + 1);
			if (!path.endsWith('/events')) return Response.json({ jobs: [] });
			return new Response(
				new ReadableStream<Uint8Array>({
					start(controller) {
						writers.set(path, controller);
						controller.enqueue(encoder.encode('event: snapshot\ndata: {"jobs":[]}\n\n'));
					},
					cancel() {
						cancels++;
					}
				}),
				{ headers: { 'Content-Type': 'text/event-stream' } }
			);
		}
	});
	Object.assign(globalThis, {
		window: Object.assign(new EventTarget(), { location: { origin: server.url.origin } })
	});
	const streams = connectDesktopJobs();
	const id = '11111111-1111-4111-8111-111111111111';
	const remote = `/api/v1/desktop/servers/${id}/api/v1/jobs/events`;
	try {
		streams.syncServers([{ id, name: 'Pico', url: 'https://example.test' }]);
		await settle();
		const push = (path: string, value: ReturnType<typeof job>) =>
			writers.get(path)?.enqueue(encoder.encode(`event: job\ndata: ${JSON.stringify(value)}\n\n`));
		push(remote, job());
		push('/api/v1/desktop/transfers/events', job('transfer-proof'));
		await settle();
		assert.equal(jobsStore.jobs.get(`@remote/${id}/proof`)?.progress, 5);
		assert.equal(jobsStore.jobs.get('transfer-proof')?.progress, 5);
		push(remote, job('proof', 'running', 60));
		await settle();
		assert.equal(jobsStore.jobs.get(`@remote/${id}/proof`)?.progress, 60);
		let finishes = 0;
		jobsStore.onFinished = () => finishes++;
		push(remote, job('proof', 'completed', 100));
		await settle();
		assert.equal(finishes, 1);
		await new Promise((resolve) => setTimeout(resolve, 2100));
		assert.equal(
			[...counts].some(([path]) => !path.endsWith('/events')),
			false,
			'healthy streams never list jobs'
		);
		streams.syncServers([{ id, name: 'Pico', url: 'https://example.test' }]);
		await settle();
		assert.equal(counts.get(remote), 1, 'catalog refresh does not reconnect the stream');
		invalidateRemoteServers();
		await settle();
		assert.equal(counts.get(remote), 2, 'pairing changes replace the authenticated stream');
		streams.syncServers([]);
		await settle();
		assert.ok(cancels >= 2);
	} finally {
		streams.close();
		await settle();
		server.stop(true);
	}
});

test('legacy sources discover once and recover known active jobs without idle list polling', async () => {
	const paths: string[] = [];
	const server = Bun.serve({
		port: 0,
		fetch(request) {
			const path = new URL(request.url).pathname;
			paths.push(path);
			if (path.endsWith('/events')) return new Response(null, { status: 404 });
			if (path === '/api/v1/jobs') return Response.json({ jobs: [job('legacy')] });
			if (path === '/api/v1/jobs/legacy') return Response.json(job('legacy', 'completed', 100));
			return Response.json({ jobs: [] });
		}
	});
	Object.assign(globalThis, {
		window: Object.assign(new EventTarget(), { location: { origin: server.url.origin } })
	});
	const streams = connectDesktopJobs();
	try {
		await settle();
		assert.equal(jobsStore.jobs.get('legacy')?.state, 'running');
		await new Promise((resolve) => setTimeout(resolve, 2200));
		assert.equal(jobsStore.jobs.get('legacy')?.state, 'completed');
		assert.equal(paths.filter((path) => path === '/api/v1/jobs').length, 1);
		assert.equal(paths.filter((path) => path === '/api/v1/jobs/legacy').length, 1);
		assert.equal(
			paths.filter((path) => path === '/api/v1/jobs/events').length,
			1,
			'unsupported streams do not retry forever'
		);
	} finally {
		streams.close();
		server.stop(true);
	}
});

test('a dropped stream reconciles completion from the next snapshot with no list request', async () => {
	let connects = 0,
		lists = 0;
	const server = Bun.serve({
		port: 0,
		fetch(request) {
			const path = new URL(request.url).pathname;
			if (!path.endsWith('/events')) {
				lists++;
				return Response.json({ jobs: [] });
			}
			const local = path === '/api/v1/jobs/events';
			if (local) connects++;
			const attempt = connects;
			return new Response(
				new ReadableStream<Uint8Array>({
					start(controller) {
						controller.enqueue(
							encoder.encode(
								`event: snapshot\ndata: ${JSON.stringify({ jobs: local ? [job('missed', attempt === 1 ? 'running' : 'completed', attempt === 1 ? 20 : 100)] : [] })}\n\n`
							)
						);
						if (local && attempt === 1) controller.close();
					}
				}),
				{ headers: { 'Content-Type': 'text/event-stream' } }
			);
		}
	});
	Object.assign(globalThis, {
		window: Object.assign(new EventTarget(), { location: { origin: server.url.origin } })
	});
	const streams = connectDesktopJobs();
	let finishes = 0;
	jobsStore.onFinished = () => finishes++;
	try {
		await new Promise((resolve) => setTimeout(resolve, 1200));
		assert.equal(connects, 2);
		assert.equal(lists, 0);
		assert.equal(jobsStore.jobs.get('missed')?.state, 'completed');
		assert.equal(finishes, 1);
	} finally {
		streams.close();
		server.stop(true);
	}
});
