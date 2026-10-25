import { untrack } from 'svelte';
import { directoryUrl, listDirectory, listRoots, getDriveStats, search } from '#lib/api/files.js';
import { earlyStart, type EarlyStartSource } from '#lib/api/earlyStart.js';
import { isRemotePath } from '#lib/api/remotePaths.js';
import { CONFIG } from '#lib/config.js';

type Confirmation =
	{ kind: 'idle' } | { kind: 'pending' } | { kind: 'ready' } | { kind: 'failed'; error: Error };

/** Aborting also cancels retry backoff and offline waits. */
function waitForRequest(signal: AbortSignal, delay = 0): Promise<void> {
	return new Promise((resolve) => {
		let stopOnline = () => {};
		const finish = () => {
			clearTimeout(timer);
			signal.removeEventListener('abort', finish);
			stopOnline();
			resolve();
		};
		const online = () => {
			if (typeof navigator === 'undefined' || navigator.onLine !== false) finish();
		};
		signal.addEventListener('abort', finish, { once: true });
		const timer = setTimeout(() => {
			if (typeof navigator !== 'undefined' && navigator.onLine === false) {
				window.addEventListener('online', online);
				stopOnline = () => window.removeEventListener('online', online);
			} else finish();
		}, delay);
		if (signal.aborted) finish();
	});
}

/** Private request mechanics; the owner below supplies all browse policy. */
class ConfirmationRequest<T> {
	private snapshot = $state.raw<T>();
	private state = $state<Confirmation>({ kind: 'idle' });
	confirmed = $state(0);
	readers = 0;
	idleSince = Date.now();
	private disposed = false;
	private generation = 0;
	private request: { controller: AbortController; done: Promise<void> } | undefined;

	constructor(private readonly fetch: (signal: AbortSignal) => Promise<T>) {}

	get value() {
		return this.state.kind !== 'idle' ? this.snapshot : undefined;
	}
	get error() {
		return this.state.kind === 'failed' ? this.state.error : null;
	}
	get pending() {
		return this.state.kind === 'pending';
	}

	acquire(): () => void {
		this.readers++;
		void this.load();
		let released = false;
		return () =>
			untrack(() => {
				if (released) return;
				released = true;
				if (--this.readers === 0) {
					this.idleSince = Date.now();
					this.cancel();
				}
			});
	}

	load(): Promise<void> {
		if (this.disposed) return Promise.resolve();
		if (this.request) return this.request.done;
		const controller = new AbortController();
		const { signal } = controller;
		this.state = { kind: 'pending' };
		const done = (async () => {
			// Yield so cancellation and publication always see the installed request identity.
			await Promise.resolve();
			try {
				let generation: number;
				do {
					generation = this.generation;
					for (let attempt = 0; ; attempt++) {
						await waitForRequest(signal, attempt ? 1000 : 0);
						if (signal.aborted) return;
						try {
							const value = await this.fetch(signal);
							if (signal.aborted) return;
							if (generation === this.generation) {
								this.snapshot = value;
								this.confirmed = Date.now() > this.confirmed ? Date.now() : this.confirmed + 1;
								this.state = { kind: 'ready' };
							}
							break;
						} catch (error) {
							if (signal.aborted) return;
							if (generation !== this.generation) break;
							if (attempt === 1) throw error;
						}
					}
				} while (generation !== this.generation && !signal.aborted);
			} catch (error) {
				if (!signal.aborted)
					this.state = {
						kind: 'failed',
						error: error instanceof Error ? error : new Error(String(error))
					};
			} finally {
				if (this.request?.controller === controller) this.request = undefined;
			}
		})();
		this.request = { controller, done };
		return done;
	}

	invalidate(): Promise<void> {
		this.generation++;
		if (this.request) return this.request.done;
		if (this.readers) return this.load();
		this.snapshot = undefined;
		this.state = { kind: 'idle' };
		this.confirmed = 0;
		return Promise.resolve();
	}

	private cancel(): void {
		this.request?.controller.abort();
		this.request = undefined;
		this.state = { kind: 'idle' };
	}

	dispose(): void {
		this.disposed = true;
		this.cancel();
		this.snapshot = undefined;
	}
}

/** Source-qualified folders and catalogs shared by panes, prefetch and change producers. */
export class BrowseRequests {
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- Cache membership is private; only snapshots are reactive.
	private folders = new Map<
		string,
		ConfirmationRequest<Awaited<ReturnType<typeof listDirectory>>>
	>();
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- Query text changes must not subscribe effects to cache membership.
	private searches = new Map<
		string,
		{ path: string; request: ConfirmationRequest<Awaited<ReturnType<typeof search>>> }
	>();
	private roots = new ConfirmationRequest((signal) => listRoots(undefined, signal));
	private stats = new ConfirmationRequest((signal) => getDriveStats(undefined, signal));
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- Listener registration is not render state.
	private changes = new Set<(event: 'changed' | 'reset') => Promise<unknown> | void>();
	private session = $state(0);
	private enabled = true;

	constructor(private readonly early: EarlyStartSource = earlyStart) {}

	folder(path: string) {
		void this.session;
		let entry = this.folders.get(path);
		if (!entry) {
			entry = new ConfirmationRequest(async (signal) => {
				const early = await this.early.takeDirectory(directoryUrl(path));
				if (signal.aborted) throw signal.reason;
				return early ?? listDirectory(path, signal);
			});
			this.folders.set(path, entry);
		}
		return entry;
	}

	catalog() {
		void this.session;
		return { roots: this.roots, stats: this.stats };
	}

	matches(path: string, query: string) {
		void this.session;
		const key = JSON.stringify([path, query]);
		let entry = this.searches.get(key);
		if (!entry) {
			entry = { path, request: new ConfirmationRequest((signal) => search(path, query, signal)) };
			this.searches.set(key, entry);
		}
		return entry.request;
	}

	observe(path: string): () => void {
		this.prune();
		return this.enabled ? this.folder(path).acquire() : () => {};
	}

	observeCatalog(atRoot: boolean): () => void {
		if (!this.enabled) return () => {};
		const releaseRoots = this.roots.acquire();
		const releaseStats = atRoot ? this.stats.acquire() : () => {};
		return () => {
			releaseRoots();
			releaseStats();
		};
	}

	observeSearch(path: string, query: string): () => void {
		this.prune();
		return this.enabled ? this.matches(path, query).acquire() : () => {};
	}

	prefetch(path: string): Promise<void> {
		this.prune();
		const entry = this.folder(path);
		return this.enabled && Date.now() - entry.confirmed > CONFIG.query.prefetchStaleTimeMs
			? entry.load()
			: Promise.resolve();
	}

	async changed(path?: string): Promise<void> {
		if (!this.enabled) return;
		const work: Promise<unknown>[] = [];
		for (const [folder, entry] of this.folders)
			if (path === undefined || folder === path) work.push(entry.invalidate());
		// A folder change may alter recursive search results in a different pane.
		for (const { request } of this.searches.values()) work.push(request.invalidate());
		if (path === undefined) work.push(this.roots.invalidate(), this.stats.invalidate());
		for (const changed of this.changes) work.push(Promise.resolve(changed('changed')));
		await Promise.all(work);
	}

	onChange(changed: (event: 'changed' | 'reset') => Promise<unknown> | void): () => void {
		this.changes.add(changed);
		return () => this.changes.delete(changed);
	}

	reset(): void {
		this.enabled = false;
		for (const entry of this.folders.values()) entry.dispose();
		for (const { request } of this.searches.values()) request.dispose();
		this.roots.dispose();
		this.stats.dispose();
		this.folders.clear();
		this.searches.clear();
		this.roots = new ConfirmationRequest((signal) => listRoots(undefined, signal));
		this.stats = new ConfirmationRequest((signal) => getDriveStats(undefined, signal));
		this.session++;
		for (const changed of this.changes) void changed('reset');
	}

	startSession(): void {
		this.enabled = true;
		this.session++;
	}

	/** One lifecycle per workspace, rather than per pane. */
	connect(): () => void {
		const confirm = (poll = false) => {
			if (!this.enabled || document.visibilityState === 'hidden') return;
			const load = (entry: ConfirmationRequest<unknown>) => {
				if (entry.readers) void entry.load();
			};
			load(this.roots);
			if (!poll) load(this.stats);
			for (const [path, entry] of this.folders) if (!poll || isRemotePath(path)) load(entry);
			for (const { path, request } of this.searches.values())
				if (!poll || isRemotePath(path)) load(request);
			this.prune();
		};
		const focus = () => confirm();
		window.addEventListener('focus', focus);
		window.addEventListener('online', focus);
		document.addEventListener('visibilitychange', focus);
		const timer = setInterval(() => confirm(true), 15_000);
		return () => {
			clearInterval(timer);
			window.removeEventListener('focus', focus);
			window.removeEventListener('online', focus);
			document.removeEventListener('visibilitychange', focus);
		};
	}

	private prune(): void {
		const evict = <T>(
			entries: Map<string, T>,
			request: (entry: T) => ConfirmationRequest<unknown>
		) => {
			const inactive = [...entries]
				.filter(([, value]) => request(value).readers === 0)
				.sort((a, b) => request(a[1]).idleSince - request(b[1]).idleSince);
			for (const [index, [key, entry]] of inactive.entries()) {
				if (inactive.length - index <= 32 && Date.now() - request(entry).idleSince < 300_000) break;
				request(entry).dispose();
				entries.delete(key);
			}
		};
		evict(this.folders, (entry) => entry);
		evict(this.searches, (entry) => entry.request);
	}
}

export const browseRequests = new BrowseRequests();
