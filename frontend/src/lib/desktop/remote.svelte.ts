export type RemoteState<T> =
	{ kind: 'loading' } | { kind: 'ready'; value: T } | { kind: 'failed'; message: string };

export type ActionState<A extends string> =
	| { kind: 'idle' }
	| { kind: 'running'; id: string; action: A }
	| { kind: 'failed'; id: string; action: A; message: string };

function errorMessage(error: unknown, fallback: string): string {
	if (error instanceof Error && error.message.trim() !== '') return error.message;
	return fallback;
}

function isAbortError(error: unknown): boolean {
	return error instanceof Error && error.name === 'AbortError';
}

/**
 * A server snapshot a desktop panel shows. A failed refresh keeps the last real
 * value and reports beside it; an older response never replaces a newer one.
 */
export class RemoteValue<T> {
	state = $state<RemoteState<T>>({ kind: 'loading' });
	refreshing = $state(false);
	refreshError = $state<string | null>(null);

	#fetch: (signal?: AbortSignal) => Promise<T>;
	#fallback: string;
	#revision = 0;

	constructor(fetch: (signal?: AbortSignal) => Promise<T>, fallback: string) {
		this.#fetch = fetch;
		this.#fallback = fallback;
	}

	get value(): T | null {
		return this.state.kind === 'ready' ? this.state.value : null;
	}

	async load(signal?: AbortSignal): Promise<void> {
		const revision = ++this.#revision;
		this.refreshError = null;
		if (this.state.kind !== 'ready') this.state = { kind: 'loading' };
		this.refreshing = true;
		try {
			const value = await this.#fetch(signal);
			if (revision === this.#revision) this.state = { kind: 'ready', value };
		} catch (error) {
			if (revision !== this.#revision || isAbortError(error)) return;
			const message = errorMessage(error, this.#fallback);
			if (this.state.kind === 'ready') this.refreshError = message;
			else this.state = { kind: 'failed', message };
		} finally {
			if (revision === this.#revision) this.refreshing = false;
		}
	}

	/** Takes a snapshot the server returned from an action; in-flight loads are older. */
	set(value: T): void {
		this.#revision += 1;
		this.refreshing = false;
		this.refreshError = null;
		this.state = { kind: 'ready', value };
	}
}

/** One mutation at a time, keyed by the item it acts on. */
export class PendingAction<A extends string> {
	state = $state<ActionState<A>>({ kind: 'idle' });

	#fallback: string;

	constructor(fallback: string) {
		this.#fallback = fallback;
	}

	get busy(): boolean {
		return this.state.kind === 'running';
	}

	get error(): string | null {
		return this.state.kind === 'failed' ? this.state.message : null;
	}

	isRunning(id: string, action: A): boolean {
		return this.state.kind === 'running' && this.state.id === id && this.state.action === action;
	}

	reset(): void {
		if (this.state.kind === 'failed') this.state = { kind: 'idle' };
	}

	/** Resolves false when another action is running or the operation fails. */
	async run(id: string, action: A, operation: () => Promise<void>): Promise<boolean> {
		if (this.state.kind === 'running') return false;
		this.state = { kind: 'running', id, action };
		try {
			await operation();
			this.state = { kind: 'idle' };
			return true;
		} catch (error) {
			this.state = { kind: 'failed', id, action, message: errorMessage(error, this.#fallback) };
			return false;
		}
	}
}
