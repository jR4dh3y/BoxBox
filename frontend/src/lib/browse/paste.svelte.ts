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
		/** Pastes into the open folder, or into `into`, such as a path crumb or a sidebar place. */
		async paste(into?: string) {
			const here = into === undefined || into === location.path;
			if (loading || !location.path || (here && !data.canCreate)) return;
			const destination = into ?? location.path;
			const opened = location.path;
			const revision = getBrowseClipboardPublication().revision;
			loading = true;
			try {
				if (!controller) {
					const module = await import('./pasteRuntime.svelte');
					controller = module.useBrowsePaste(location, data);
				}
				if (opened !== location.path || revision !== getBrowseClipboardPublication().revision) {
					toastStore.info(
						'The folder or clipboard changed. Paste again to use the current selection.'
					);
					return;
				}
				await controller.paste(destination);
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
