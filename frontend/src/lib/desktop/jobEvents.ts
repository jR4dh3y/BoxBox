import { api, buildUrl, ApiRequestError } from '#lib/api/client.js';
import { getJob, parseJob, parseJobs } from '#lib/api/jobs.js';
import {
	onRemoteServersChanged,
	parseRemotePath,
	type PathReference,
	type RemoteServer
} from '#lib/api/remotePaths.js';
import { record, string } from '#lib/api/parse.js';
import { jobsStore } from '#lib/stores/jobs.svelte.js';

/** Reconcile each source independently; healthy sources never poll jobs. */
class JobStream {
	private controller = new AbortController();
	private retry: ReturnType<typeof setTimeout> | undefined;
	private fallback: ReturnType<typeof setInterval> | undefined;
	private stopped = false;
	private attempts = 0;
	private recovering = false;
	private unavailable = new Set<string>();
	private blocked = new Set<string>();
	healthy = false;

	constructor(
		private readonly endpoint: string,
		private readonly source: PathReference,
		private readonly owns: (id: string) => boolean,
		private readonly sources?: Map<string, PathReference>
	) {
		if (sources) {
			this.unavailable = new Set(sources.keys());
			this.fallback = setInterval(() => void this.recover(), 2000);
		}
		void this.connect();
	}

	private async recover() {
		if (this.recovering || this.stopped || (typeof document !== 'undefined' && document.hidden))
			return;
		this.recovering = true;
		try {
			for (const job of jobsStore.active.filter((job) => {
				const source = parseRemotePath(job.id);
				return (
					this.owns(job.id) &&
					(!this.sources ||
						(source.kind === 'remote' &&
							this.unavailable.has(source.serverId) &&
							!this.blocked.has(source.serverId)))
				);
			})) {
				try {
					const current = await getJob(job.id, this.controller.signal);
					if (!this.stopped) jobsStore.upsertJob(current);
				} catch (error) {
					if (this.stopped) return;
					if (error instanceof ApiRequestError && error.status === 404)
						jobsStore.jobs.delete(job.id);
					// A failed source must not skip recovery for the next source.
				}
			}
		} finally {
			this.recovering = false;
		}
	}

	private async connect() {
		let unsupported = false;
		let blocked = false;
		try {
			const response = await fetch(
				buildUrl(this.sources ? this.endpoint : `${this.endpoint}/events`),
				{
					signal: this.controller.signal,
					credentials: 'same-origin'
				}
			);
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
						if (event === 'source' && this.sources) {
							const envelope = record(JSON.parse(data), 'remote job event');
							const id = string(envelope.serverId, 'remote job source');
							const source = this.sources.get(id);
							if (!source) throw new Error('Unknown remote job source');
							if (envelope.kind === 'snapshot') {
								jobsStore.reconcileJobs(parseJobs(envelope.data, source), (jobId) =>
									jobId.startsWith(`@remote/${id}/`)
								);
								this.unavailable.delete(id);
								this.blocked.delete(id);
								this.healthy = this.unavailable.size === 0;
								this.attempts = 0;
							} else if (envelope.kind === 'job') {
								jobsStore.receiveJob(parseJob(envelope.data, source));
							} else if (envelope.kind === 'unavailable' || envelope.kind === 'blocked') {
								if (envelope.kind === 'blocked') this.blocked.add(id);
								this.unavailable.add(id);
								this.healthy = false;
							} else throw new Error('Invalid remote job event');
						} else if (event === 'snapshot' && !this.sources) {
							jobsStore.reconcileJobs(parseJobs(JSON.parse(data), this.source), this.owns);
							this.healthy = true;
							this.attempts = 0;
							clearInterval(this.fallback);
							this.fallback = undefined;
						} else if (event === 'job' && !this.sources)
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
		if (this.sources) this.unavailable = new Set(this.sources.keys());
		if (blocked) return;
		// Older deployments lack events. Discover their existing work once, then only check known active jobs.
		if (unsupported && !this.sources) {
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
	let remotes: JobStream | undefined;
	let sourceKey = '';
	const syncServers = (next: RemoteServer[]) => {
		for (const old of servers)
			if (!next.some((server) => server.id === old.id))
				jobsStore.reconcileJobs([], (id) => id.startsWith(`@remote/${old.id}/`));
		servers = next;
		const key = next
			.map((server) => server.id)
			.sort()
			.join(',');
		if (key === sourceKey) return;
		sourceKey = key;
		remotes?.close();
		remotes = undefined;
		if (!next.length) return;
		const sources = new Map<string, PathReference>();
		const query = new URLSearchParams();
		for (const server of next) {
			sources.set(server.id, { kind: 'remote', serverId: server.id, path: '' });
			query.append('server', server.id);
		}
		remotes = new JobStream(
			`/desktop/servers/events?${query}`,
			local,
			(id) => {
				const source = parseRemotePath(id);
				return source.kind === 'remote' && sources.has(source.serverId);
			},
			sources
		);
	};
	const restartRemotes = (failedOnly = false) => {
		if (failedOnly && remotes?.healthy) return;
		sourceKey = '';
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
			remotes?.close();
			for (const stream of streams.values()) stream.close();
			streams.clear();
			jobsStore.reset();
		}
	};
}
