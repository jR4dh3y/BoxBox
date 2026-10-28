import { api, buildUrl, ApiRequestError } from '#lib/api/client.js';
import { getJob, parseJob, parseJobs } from '#lib/api/jobs.js';
import {
	onRemoteServersChanged,
	parseRemotePath,
	sourceEndpoint,
	type PathReference,
	type RemoteServer
} from '#lib/api/remotePaths.js';
import { jobsStore } from '#lib/stores/jobs.svelte.js';

/** One source owns its stream, reconnects and recovery. Healthy streams never poll jobs. */
class JobStream {
	private controller = new AbortController();
	private retry: ReturnType<typeof setTimeout> | undefined;
	private fallback: ReturnType<typeof setInterval> | undefined;
	private stopped = false;
	private attempts = 0;
	private recovering = false;
	healthy = false;

	constructor(
		private readonly endpoint: string,
		private readonly source: PathReference,
		private readonly owns: (id: string) => boolean
	) {
		void this.connect();
	}

	private async recover() {
		if (this.recovering || this.stopped || (typeof document !== 'undefined' && document.hidden))
			return;
		this.recovering = true;
		try {
			for (const job of jobsStore.active.filter((job) => this.owns(job.id))) {
				try {
					const current = await getJob(job.id, this.controller.signal);
					if (!this.stopped) jobsStore.upsertJob(current);
				} catch (error) {
					if (!this.stopped && error instanceof ApiRequestError && error.status === 404)
						jobsStore.jobs.delete(job.id);
					else throw error;
				}
			}
		} catch {
			/* Reconnect reconciles jobs after the source becomes available. */
		} finally {
			this.recovering = false;
		}
	}

	private async connect() {
		let unsupported = false;
		let blocked = false;
		try {
			const response = await fetch(buildUrl(`${this.endpoint}/events`), {
				signal: this.controller.signal,
				credentials: 'same-origin'
			});
			unsupported = response.status === 404 || response.status === 405;
			if (
				!response.ok ||
				!response.headers.get('Content-Type')?.startsWith('text/event-stream') ||
				!response.body
			) {
				blocked = response.status === 401 || response.status === 403;
				if (response.headers.get('Content-Type')?.includes('application/json')) {
					const error: unknown = await response.json();
					if (
						typeof error === 'object' &&
						error !== null &&
						'code' in error &&
						error.code === 'REMOTE_AUTH_REQUIRED'
					)
						blocked = true;
				}
				await response.body?.cancel().catch(() => {});
				throw new Error('Job events unavailable');
			}
			const reader = response.body.getReader();
			const decoder = new TextDecoder();
			let buffer = '';
			try {
				while (!this.stopped) {
					const { value, done } = await reader.read();
					if (done || this.stopped) break;
					buffer += decoder.decode(value, { stream: true });
					if (buffer.length > 4_194_304) throw new Error('Job event too large');
					let boundary: number;
					while ((boundary = buffer.indexOf('\n\n')) >= 0) {
						const block = buffer.slice(0, boundary);
						buffer = buffer.slice(boundary + 2);
						const lines = block.split('\n');
						const event = lines.find((line) => line.startsWith('event: '))?.slice(7);
						const data = lines
							.filter((line) => line.startsWith('data: '))
							.map((line) => line.slice(6))
							.join('\n');
						if (event === 'snapshot') {
							jobsStore.reconcileJobs(parseJobs(JSON.parse(data), this.source), this.owns);
							this.healthy = true;
							this.attempts = 0;
							clearInterval(this.fallback);
							this.fallback = undefined;
						} else if (event === 'job')
							jobsStore.receiveJob(parseJob(JSON.parse(data), this.source));
					}
				}
			} finally {
				await reader.cancel().catch(() => {});
			}
		} catch {
			/* Failure switches only this source to active-job recovery. */
		}
		if (this.stopped) return;
		this.healthy = false;
		if (blocked) return;
		// Older deployments lack events. Discover their existing work once, then only check known active jobs.
		if (unsupported) {
			try {
				const snapshot = parseJobs(
					await api.get<unknown>(this.endpoint, undefined, this.controller.signal),
					this.source
				);
				if (!this.stopped) jobsStore.reconcileJobs(snapshot, this.owns);
			} catch {
				/* The next explicit reconnect retries discovery. */
			}
		}
		if (this.stopped) return;
		this.fallback ??= setInterval(() => void this.recover(), 2000);
		if (!unsupported)
			this.retry = setTimeout(
				() => void this.connect(),
				Math.min(1000 * 2 ** this.attempts++, 30_000)
			);
	}

	close() {
		this.stopped = true;
		this.controller.abort();
		clearTimeout(this.retry);
		clearInterval(this.fallback);
	}
}

/** The workspace owns local and remote job lifetimes, shared by all panes. */
export function connectDesktopJobs() {
	const local: PathReference = { kind: 'local', path: '' };
	const streams = new Map<string, JobStream>();
	const start = (
		key: string,
		endpoint: string,
		source: PathReference,
		owns: (id: string) => boolean
	) => streams.set(key, new JobStream(endpoint, source, owns));
	start(
		'local',
		'/jobs',
		local,
		(id) => !id.startsWith('transfer-') && parseRemotePath(id).kind === 'local'
	);
	start('transfers', '/desktop/transfers', local, (id) => id.startsWith('transfer-'));
	let servers: RemoteServer[] = [];
	const syncServers = (next: RemoteServer[]) => {
		servers = next;
		for (const [id, stream] of streams)
			if (id !== 'local' && id !== 'transfers' && !next.some((server) => server.id === id)) {
				stream.close();
				streams.delete(id);
				jobsStore.reconcileJobs([], (jobId) => jobId.startsWith(`@remote/${id}/`));
			}
		for (const server of next)
			if (!streams.has(server.id)) {
				const source: PathReference = { kind: 'remote', serverId: server.id, path: '' };
				start(server.id, sourceEndpoint(source, '/jobs'), source, (id) => {
					const ref = parseRemotePath(id);
					return ref.kind === 'remote' && ref.serverId === server.id;
				});
			}
	};
	const restartRemotes = (failedOnly = false) => {
		for (const [id, stream] of streams)
			if (id !== 'local' && id !== 'transfers' && (!failedOnly || !stream.healthy)) {
				stream.close();
				streams.delete(id);
			}
		syncServers(servers);
	};
	const stopChanges = onRemoteServersChanged(() => restartRemotes());
	const retryFailed = () => restartRemotes(true);
	window.addEventListener('focus', retryFailed);
	window.addEventListener('online', retryFailed);
	return {
		syncServers,
		close() {
			stopChanges();
			window.removeEventListener('focus', retryFailed);
			window.removeEventListener('online', retryFailed);
			for (const stream of streams.values()) stream.close();
			streams.clear();
			jobsStore.reset();
		}
	};
}
