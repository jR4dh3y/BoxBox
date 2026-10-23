import type { useBrowsePaste as createBrowsePaste } from './pasteRuntime.svelte';
import type { BrowseData } from './data.svelte';
import type { BrowseLocation } from './location.svelte';
import type { TransferConflictDecision, TransferOperation, TransferSource } from './transfers';
import { getBrowseClipboardPublication } from './clipboardPublication';
import { toastStore } from '#lib/stores/toast.svelte.js';

/** Load transfer planning and clipboard completion only when paste is requested. */
export function useBrowsePaste(location: BrowseLocation, data: BrowseData) {
	let controller = $state.raw<ReturnType<typeof createBrowsePaste> | null>(null);
	let loading = $state(false);

	return {
		get plan() {
			return controller?.plan ?? null;
		},
		get busy() {
			return loading || (controller?.busy ?? false);
		},
		async paste() {
			if (loading || !location.path || !data.canCreate) return;
			const destination = location.path;
			const revision = getBrowseClipboardPublication().revision;
			loading = true;
			try {
				if (!controller) {
					const module = await import('./pasteRuntime.svelte');
					controller = module.useBrowsePaste(location, data);
				}
				if (
					destination !== location.path ||
					revision !== getBrowseClipboardPublication().revision
				) {
					toastStore.info(
						'The folder or clipboard changed. Paste again to use the current selection.'
					);
					return;
				}
				await controller.paste();
			} catch (error) {
				toastStore.error(error instanceof Error ? error.message : 'Unable to load paste');
			} finally {
				loading = false;
			}
		},
		async transfer(operation: TransferOperation, items: TransferSource[], destination: string) {
			if (loading) return;
			loading = true;
			try {
				if (!controller) {
					const module = await import('./pasteRuntime.svelte');
					controller = module.useBrowsePaste(location, data);
				}
				await controller.transfer(operation, items, destination);
			} catch (error) {
				toastStore.error(error instanceof Error ? error.message : 'Unable to load transfers');
			} finally {
				loading = false;
			}
		},
		apply(decisions: TransferConflictDecision[]) {
			controller?.apply(decisions);
		},
		async cancel() {
			await controller?.cancel();
		}
	};
}
