import { getJob, type Job } from '#lib/api/jobs.js';
import { SvelteSet } from 'svelte/reactivity';
import { isLocalDesktop } from '#lib/api/desktop.js';
import type { DesktopClipboardContents } from '#lib/api/desktopClipboard.js';
import { clipboardStore } from '#lib/stores/clipboard.svelte.js';
import { jobsStore } from '#lib/stores/jobs.svelte.js';
import { toastStore } from '#lib/stores/toast.svelte.js';
import type { TransferPlan, TransferQueue, TransferConflictDecision } from './transfers';
import type { BrowseData } from './data.svelte';
import type { BrowseLocation } from './location.svelte';

type PasteState =
	| { kind: 'idle' }
	| { kind: 'planning' }
	| { kind: 'conflict'; plan: TransferPlan; clipboard: DesktopClipboardContents | null }
	| { kind: 'submitting'; queue: TransferQueue };

async function finishCut(
	jobs: readonly Job[],
	clipboard: DesktopClipboardContents | null,
	sourcePaths: string[]
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
		if (clipboard) {
			const { clearBrowseCutClipboardAfterMove } = await import('./desktopClipboard');
			await clearBrowseCutClipboardAfterMove(clipboard);
		}
		if (
			clipboardStore.operation === 'cut' &&
			clipboardStore.items.length === sourcePaths.length &&
			clipboardStore.items.every((item) => sourcePaths.includes(item.path))
		)
			clipboardStore.clear();
	} catch {
		/* Keep the cut selection when completion cannot be confirmed. */
	}
}

export function useBrowsePaste(location: BrowseLocation, data: BrowseData) {
	let state = $state.raw<PasteState>({ kind: 'idle' });
	async function submit(
		plan: TransferPlan,
		decisions: TransferConflictDecision[],
		clipboard: DesktopClipboardContents | null
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
				if (plan.operation === 'move' && result.skipped.length === 0)
					void finishCut(
						result.jobs,
						clipboard,
						plan.entries.map((entry) => entry.source.path)
					);
			}
			data.refreshAll();
		} catch (error) {
			toastStore.error(error instanceof Error ? error.message : 'Paste failed');
		} finally {
			state = { kind: 'idle' };
		}
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
				let clipboard: DesktopClipboardContents | null = null;
				let items = clipboardStore.items.map(({ path, name, isDir }) => ({ path, name, isDir }));
				let operation = clipboardStore.operation;
				if (isLocalDesktop()) {
					try {
						const { readBrowseClipboard } = await import('./desktopClipboard');
						clipboard = await readBrowseClipboard();
						items = clipboard?.items ?? [];
						operation = clipboard?.operation ?? null;
					} catch (error) {
						if (!items.length) throw error;
					}
				}
				if (!items.length || !operation) {
					toastStore.info('The clipboard has no files to paste');
					state = { kind: 'idle' };
					return;
				}
				const { planTransfers } = await import('./transfers');
				const plan = await planTransfers(operation === 'cut' ? 'move' : 'copy', items, destination);
				if (plan.entries.some((entry) => entry.kind === 'conflict')) {
					state = { kind: 'conflict', plan, clipboard };
					return;
				}
				await submit(plan, [], clipboard);
			} catch (error) {
				state = { kind: 'idle' };
				toastStore.error(error instanceof Error ? error.message : 'Paste failed');
			}
		},
		apply(decisions: TransferConflictDecision[]) {
			if (state.kind === 'conflict') void submit(state.plan, decisions, state.clipboard);
		},
		async cancel() {
			if (state.kind === 'submitting') await state.queue.cancel();
			state = { kind: 'idle' };
		}
	};
}
