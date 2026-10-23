import {
	cancelJob,
	createCopyJob,
	createMoveJob,
	getJob,
	isJobActive,
	type Job
} from '#lib/api/jobs.js';
import { listDirectory, type FileList } from '#lib/api/files.js';

export type TransferOperation = 'copy' | 'move';
export type TransferConflictResolution = 'skip' | 'keep-both';

export interface TransferSource {
	path: string;
	name: string;
	isDir: boolean;
}

interface PlannedTransferBase {
	id: number;
	source: TransferSource;
	destinationPath: string;
}

export interface ReadyTransfer extends PlannedTransferBase {
	kind: 'ready';
}

export interface ConflictingTransfer extends PlannedTransferBase {
	kind: 'conflict';
	keepBothPath: string;
}

export type PlannedTransfer = ReadyTransfer | ConflictingTransfer;

export interface TransferPlan {
	operation: TransferOperation;
	destinationDirectory: string;
	entries: PlannedTransfer[];
}

export interface TransferConflictDecision {
	entryId: number;
	resolution: TransferConflictResolution;
}

export interface TransferRequest {
	operation: TransferOperation;
	source: TransferSource;
	destinationPath: string;
}

export interface ResolvedTransferPlan {
	operation: TransferOperation;
	transfers: TransferRequest[];
	skipped: TransferSource[];
}

export type DirectoryLister = (path: string, signal?: AbortSignal) => Promise<FileList>;

export interface PlanTransfersOptions {
	listDirectory?: DirectoryLister;
	signal?: AbortSignal;
}

export class TransferPlanningError extends Error {
	constructor(message: string) {
		super(message);
		this.name = 'TransferPlanningError';
	}
}

function joinVirtualPath(directory: string, name: string): string {
	if (!directory) return name;
	return directory.endsWith('/') ? `${directory}${name}` : `${directory}/${name}`;
}

function validateSource(source: TransferSource): void {
	if (!source.path || !source.name || source.name === '.' || source.name === '..') {
		throw new TransferPlanningError('A transfer item has an invalid name or path.');
	}
	if (source.name.includes('/') || source.name.includes('\\')) {
		throw new TransferPlanningError(`“${source.name}” is not a valid file name.`);
	}
}

function makeKeepBothName(name: string, occupiedNames: ReadonlySet<string>): string {
	const extensionIndex = name.lastIndexOf('.');
	const hasExtension = extensionIndex > 0 && extensionIndex < name.length - 1;
	const stem = hasExtension ? name.slice(0, extensionIndex) : name;
	const extension = hasExtension ? name.slice(extensionIndex) : '';

	for (let copyNumber = 1; copyNumber <= Number.MAX_SAFE_INTEGER; copyNumber++) {
		const candidate = `${stem} (copy ${copyNumber})${extension}`;
		if (!occupiedNames.has(candidate)) return candidate;
	}

	throw new TransferPlanningError(`Could not generate a unique name for “${name}”.`);
}

async function listAllDestinationItems(
	destinationDirectory: string,
	lister: DirectoryLister,
	signal?: AbortSignal
): Promise<string[]> {
	const result = await lister(destinationDirectory, signal);
	return result.items.map((item) => item.name);
}

/** Lists the complete destination once and assigns safe, unique keep-both names. */
export async function planTransfers(
	operation: TransferOperation,
	sources: readonly TransferSource[],
	destinationDirectory: string,
	options: PlanTransfersOptions = {}
): Promise<TransferPlan> {
	for (const source of sources) validateSource(source);
	if (sources.length === 0) return { operation, destinationDirectory, entries: [] };

	const existingNames = await listAllDestinationItems(
		destinationDirectory,
		options.listDirectory ?? listDirectory,
		options.signal
	);
	const occupiedNames = new Set(existingNames);
	const entries: PlannedTransfer[] = [];

	for (const [id, source] of sources.entries()) {
		const destinationPath = joinVirtualPath(destinationDirectory, source.name);
		if (occupiedNames.has(source.name)) {
			const keepBothName = makeKeepBothName(source.name, occupiedNames);
			entries.push({
				id,
				kind: 'conflict',
				source,
				destinationPath,
				keepBothPath: joinVirtualPath(destinationDirectory, keepBothName)
			});
			occupiedNames.add(keepBothName);
			continue;
		}

		entries.push({ id, kind: 'ready', source, destinationPath });
		occupiedNames.add(source.name);
	}

	return { operation, destinationDirectory, entries };
}

/** Applies explicit conflict choices; unresolved conflicts are skipped safely. */
export function resolveTransferPlan(
	plan: TransferPlan,
	decisions: readonly TransferConflictDecision[] = []
): ResolvedTransferPlan {
	const resolutionById = new Map(decisions.map(({ entryId, resolution }) => [entryId, resolution]));
	const transfers: TransferRequest[] = [];
	const skipped: TransferSource[] = [];

	for (const entry of plan.entries) {
		if (entry.kind === 'ready') {
			transfers.push({
				operation: plan.operation,
				source: entry.source,
				destinationPath: entry.destinationPath
			});
			continue;
		}

		if (resolutionById.get(entry.id) !== 'keep-both') {
			skipped.push(entry.source);
			continue;
		}

		transfers.push({
			operation: plan.operation,
			source: entry.source,
			destinationPath: entry.keepBothPath
		});
	}

	return { operation: plan.operation, transfers, skipped };
}

export type TransferQueueState =
	'idle' | 'submitting' | 'cancelling' | 'submitted' | 'cancelled' | 'failed';

export interface TransferQueueDependencies {
	createCopyJob: typeof createCopyJob;
	createMoveJob: typeof createMoveJob;
	cancelJob: typeof cancelJob;
	getJob: typeof getJob;
}

export interface TransferQueueOptions {
	/** Forward each real job to the existing jobs store; this module invents no progress. */
	onJobCreated?: (job: Job, transfer: TransferRequest) => void;
	onStateChange?: (state: TransferQueueState) => void;
	dependencies?: Partial<TransferQueueDependencies>;
}

export interface TransferQueueFailure {
	transfer: TransferRequest;
	error: unknown;
}

/** `submitted` means every request was sent; each Job still reports background progress. */
export type TransferQueueResult =
	| {
			state: 'submitted';
			jobs: Job[];
			skipped: TransferSource[];
	  }
	| {
			state: 'cancelled';
			jobs: Job[];
			skipped: TransferSource[];
			cancellationErrors: string[];
	  }
	| {
			state: 'failed';
			jobs: Job[];
			skipped: TransferSource[];
			failure: TransferQueueFailure;
			cancellationErrors: string[];
	  };

export interface TransferQueue {
	readonly state: TransferQueueState;
	readonly jobs: readonly Job[];
	/** Resolves when job requests finish; background work remains tracked by each returned Job. */
	start(): Promise<TransferQueueResult>;
	cancel(): Promise<void>;
}

function errorMessage(error: unknown): string {
	return error instanceof Error ? error.message : 'The background job could not be cancelled.';
}

/** Submits one background job at a time and stops on the first request error. */
export function createTransferQueue(
	plan: ResolvedTransferPlan,
	options: TransferQueueOptions = {}
): TransferQueue {
	const dependencies: TransferQueueDependencies = {
		createCopyJob,
		createMoveJob,
		cancelJob,
		getJob,
		...options.dependencies
	};
	const jobs: Job[] = [];
	const cancellationErrors: string[] = [];
	const cancellationRequests = new Map<string, Promise<void>>();
	let currentState: TransferQueueState = 'idle';
	let cancelRequested = false;
	let failure: TransferQueueFailure | undefined;
	let runPromise: Promise<TransferQueueResult> | undefined;

	function setState(state: TransferQueueState): void {
		currentState = state;
		options.onStateChange?.(state);
	}

	function cancelCreatedJob(job: Job): Promise<void> {
		const existingRequest = cancellationRequests.get(job.id);
		if (existingRequest) return existingRequest;

		const request = (async () => {
			if (!isJobActive(job)) return;
			try {
				await dependencies.cancelJob(job.id);
			} catch (cancelError) {
				try {
					const latest = await dependencies.getJob(job.id);
					if (isJobActive(latest)) cancellationErrors.push(errorMessage(cancelError));
				} catch (refreshError) {
					cancellationErrors.push(
						`${errorMessage(cancelError)} (status check failed: ${errorMessage(refreshError)})`
					);
				}
			}
		})();
		cancellationRequests.set(job.id, request);
		return request;
	}

	function result(): TransferQueueResult {
		if (failure) {
			return {
				state: 'failed',
				jobs: [...jobs],
				skipped: [...plan.skipped],
				failure,
				cancellationErrors: [...cancellationErrors]
			};
		}
		if (currentState === 'cancelled' || currentState === 'cancelling') {
			return {
				state: 'cancelled',
				jobs: [...jobs],
				skipped: [...plan.skipped],
				cancellationErrors: [...cancellationErrors]
			};
		}
		return { state: 'submitted', jobs: [...jobs], skipped: [...plan.skipped] };
	}

	async function run(): Promise<TransferQueueResult> {
		for (const transfer of plan.transfers) {
			if (cancelRequested) break;

			let job: Job;
			try {
				job =
					transfer.operation === 'copy'
						? await dependencies.createCopyJob(transfer.source.path, transfer.destinationPath)
						: await dependencies.createMoveJob(transfer.source.path, transfer.destinationPath);
			} catch (error) {
				failure = { transfer, error };
				setState('failed');
				return result();
			}

			jobs.push(job);
			try {
				options.onJobCreated?.(job, transfer);
			} catch (error) {
				failure = { transfer, error };
				setState('failed');
				return result();
			}
			if (job.state === 'failed') {
				failure = {
					transfer,
					error: new Error(job.error ?? 'The background job could not be queued.')
				};
				setState('failed');
				return result();
			}

			if (cancelRequested) await cancelCreatedJob(job);
		}

		if (cancelRequested) {
			await Promise.all(jobs.map(cancelCreatedJob));
			setState('cancelled');
		} else {
			setState('submitted');
		}
		return result();
	}

	return {
		get state() {
			return currentState;
		},
		get jobs() {
			return [...jobs];
		},
		start() {
			if (runPromise) return runPromise;
			if (currentState === 'cancelled') return Promise.resolve(result());
			if (currentState !== 'idle') return Promise.resolve(result());
			setState('submitting');
			runPromise = run();
			return runPromise;
		},
		async cancel() {
			if (currentState === 'submitted' || currentState === 'cancelled' || currentState === 'failed')
				return;

			cancelRequested = true;
			setState(currentState === 'idle' ? 'cancelled' : 'cancelling');
			await Promise.all(jobs.map(cancelCreatedJob));
			if (runPromise) await runPromise;
		}
	};
}
