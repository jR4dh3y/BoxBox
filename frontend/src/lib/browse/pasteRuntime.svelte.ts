import { getJob, type Job } from '#lib/api/jobs.js';
import { SvelteSet } from 'svelte/reactivity';
import { isNativePath } from '#lib/api/desktop.js';
import {
	clearDesktopCutClipboard,
	getDesktopClipboard,
	type DesktopClipboardContents
} from '#lib/api/desktopClipboard.js';
import { clipboardStore } from '#lib/stores/clipboard.svelte.js';
import {
	clearBrowseClipboardPublication,
	getBrowseClipboardPublication,
	isCurrentBrowseClipboardSelection,
	shouldReadNativeBrowseClipboard
} from './clipboardPublication';
import { jobsStore } from '#lib/stores/jobs.svelte.js';
import { toastStore } from '#lib/stores/toast.svelte.js';
import type {
	TransferPlan,
	TransferQueue,
	TransferConflictDecision,
	TransferOperation,
	TransferSource
} from './transfers';
import type { BrowseData } from './data.svelte';
import type { BrowseLocation } from './location.svelte';

type PasteState =
	| { kind: 'idle' }
	| { kind: 'planning' }
	| {
			kind: 'conflict';
			plan: TransferPlan;
			clipboard: DesktopClipboardContents | null;
			selectionId: number | null;
	  }
	| { kind: 'submitting'; queue: TransferQueue };

async function finishCut(
	jobs: readonly Job[],
	clipboard: DesktopClipboardContents | null,
	sourcePaths: string[],
	selectionId: number | null
) {
	const pending = new SvelteSet(jobs.map((job) => job.id));
	try {
		while (pending.size) {
			for (const id of pending) {
				const job = await getJob(id);
				jobsStore.upsertJob(job);
				if (job.state === 'failed' || job.state === 'cancelled') return;
				if (job.state === 'completed') pending.delete(id);
			}
			if (pending.size) await new Promise((resolve) => setTimeout(resolve, 1500));
		}
		if (
			clipboard?.operation === 'cut' &&
			(selectionId === null || isCurrentBrowseClipboardSelection(selectionId))
		) {
			await clearDesktopCutClipboard(clipboard.items.map((item) => item.path));
		}
		if (
			selectionId !== null &&
			isCurrentBrowseClipboardSelection(selectionId) &&
			clipboardStore.operation === 'cut' &&
			clipboardStore.items.length === sourcePaths.length &&
			clipboardStore.items.every((item) => sourcePaths.includes(item.path))
		) {
			clipboardStore.clear();
			clearBrowseClipboardPublication(selectionId);
		}
	} catch {
		/* Keep the cut selection when completion cannot be confirmed. */
	}
}

interface PasteSource {
	/** Native clipboard contents, or null when pasting the in-app selection. */
	clipboard: DesktopClipboardContents | null;
	selectionId: number | null;
	items: DesktopClipboardContents['items'];
	operation: DesktopClipboardContents['operation'] | null;
}

function inAppPasteSource(): PasteSource {
	const publication = getBrowseClipboardPublication();
	return {
		clipboard: null,
		selectionId: publication.kind === 'external' ? null : publication.revision,
		items: clipboardStore.items.map(({ path, name, isDir }) => ({ path, name, isDir })),
		operation: clipboardStore.operation
	};
}

/** Native contents win unless an in-app copy or cut started while they were being read. */
async function readPasteSource(destination: string): Promise<PasteSource> {
	if (!isNativePath(destination) || !shouldReadNativeBrowseClipboard()) return inAppPasteSource();
	const started = getBrowseClipboardPublication();
	const unchanged = () => {
		const current = getBrowseClipboardPublication();
		return current.revision === started.revision && current.kind === started.kind;
	};
	try {
		const clipboard = await getDesktopClipboard();
		if (!unchanged()) return inAppPasteSource();
		return {
			clipboard,
			selectionId: started.kind === 'external' ? null : started.revision,
			items: clipboard?.items ?? [],
			operation: clipboard?.operation ?? null
		};
	} catch (error) {
		const fallback = inAppPasteSource();
		if (unchanged() && fallback.items.length === 0) throw error;
		return fallback;
	}
}

export function useBrowsePaste(location: BrowseLocation, data: BrowseData) {
	let state = $state.raw<PasteState>({ kind: 'idle' });
	async function submit(
		plan: TransferPlan,
		decisions: TransferConflictDecision[],
		clipboard: DesktopClipboardContents | null,
		selectionId: number | null
	) {
		state = { kind: 'planning' };
		try {
			const { resolveTransferPlan, createTransferQueue } = await import('./transfers');
			const resolved = resolveTransferPlan(plan, decisions);
			const queue = createTransferQueue(resolved, {
				onJobCreated: (job) => jobsStore.upsertJob(job)
			});
			state = { kind: 'submitting', queue };
			const result = await queue.start();
			if (result.state === 'failed') throw result.failure.error;
			if (result.state === 'submitted' && result.jobs.length) {
				toastStore.success(
					`${result.jobs.length} transfer${result.jobs.length === 1 ? '' : 's'} queued`
				);
				// Only a pasted cut has a clipboard to clear once the moves finish.
				if (
					plan.operation === 'move' &&
					result.skipped.length === 0 &&
					(clipboard !== null || selectionId !== null)
				)
					void finishCut(
						result.jobs,
						clipboard,
						plan.entries.map((entry) => entry.source.path),
						selectionId
					);
			}
			data.refreshAll();
		} catch (error) {
			toastStore.error(error instanceof Error ? error.message : 'Paste failed');
		} finally {
			state = { kind: 'idle' };
		}
	}
	async function planAndSubmit(
		operation: TransferOperation,
		items: TransferSource[],
		destination: string,
		clipboard: DesktopClipboardContents | null,
		selectionId: number | null
	) {
		const { planTransfers } = await import('./transfers');
		const plan = await planTransfers(operation, items, destination);
		if (plan.entries.some((entry) => entry.kind === 'conflict')) {
			state = { kind: 'conflict', plan, clipboard, selectionId };
			return;
		}
		await submit(plan, [], clipboard, selectionId);
	}
	return {
		get plan() {
			return state.kind === 'conflict' ? state.plan : null;
		},
		get busy() {
			return state.kind === 'planning' || state.kind === 'submitting';
		},
		async paste() {
			if (state.kind !== 'idle' || !location.path || !data.canCreate) return;
			const destination = location.path;
			state = { kind: 'planning' };
			try {
				const { clipboard, selectionId, items, operation } = await readPasteSource(destination);
				if (!items.length || !operation) {
					toastStore.info('The clipboard has no files to paste');
					state = { kind: 'idle' };
					return;
				}
				await planAndSubmit(
					operation === 'cut' ? 'move' : 'copy',
					items,
					destination,
					clipboard,
					selectionId
				);
			} catch (error) {
				state = { kind: 'idle' };
				toastStore.error(error instanceof Error ? error.message : 'Paste failed');
			}
		},
		/** Copies or moves dropped items; conflicts ask the same question as paste. */
		async transfer(operation: TransferOperation, items: TransferSource[], destination: string) {
			if (state.kind !== 'idle' || items.length === 0) return;
			state = { kind: 'planning' };
			try {
				await planAndSubmit(operation, items, destination, null, null);
			} catch (error) {
				state = { kind: 'idle' };
				toastStore.error(
					error instanceof Error
						? error.message
						: `${operation === 'move' ? 'Move' : 'Copy'} failed`
				);
			}
		},
		apply(decisions: TransferConflictDecision[]) {
			if (state.kind === 'conflict')
				void submit(state.plan, decisions, state.clipboard, state.selectionId);
		},
		async cancel() {
			if (state.kind === 'submitting') await state.queue.cancel();
			state = { kind: 'idle' };
		}
	};
}
