import { onDestroy } from 'svelte';
import { SvelteSet } from 'svelte/reactivity';
import { isNativePath } from '#lib/api/desktop.js';
import type { FileInfo } from '#lib/api/files.js';
import { clipboardStore } from '#lib/stores/clipboard.svelte.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { toastStore } from '#lib/stores/toast.svelte.js';
import type { BrowseActions } from './actions.svelte';
import type { BrowseData } from './data.svelte';
import type { BrowseLocation } from './location.svelte';
import { useBrowsePaste } from './paste.svelte';
import { browseRequests } from './requests.svelte';
import type { BrowseSelection } from './selection.svelte';

/** Keep folder navigation and row decoration available before the first file command. */
export function useFileActions(
	location: BrowseLocation,
	data: BrowseData,
	selection: BrowseSelection
) {
	const paste = useBrowsePaste(location, data);
	let controller = $state.raw<BrowseActions | null>(null);
	let loading: Promise<BrowseActions> | undefined;
	let active = true;
	onDestroy(() => (active = false));

	const favoritePaths = $derived(
		new SvelteSet(settingsStore.current.favoriteFolders.map((folder) => folder.path))
	);
	const cutPaths = $derived(
		new SvelteSet(
			clipboardStore.operation === 'cut' ? clipboardStore.items.map((item) => item.path) : []
		)
	);

	async function run(command: (actions: BrowseActions) => void | Promise<void>) {
		const path = location.path;
		const signal = browseRequests.sessionSignal;
		try {
			const actions =
				controller ??
				(await (loading ??= import('./actions.svelte.js').then(
					({ useBrowseActions }) =>
						(controller = useBrowseActions(location, data, selection, paste))
				)));
			if (active && !signal.aborted && location.path === path) await command(actions);
		} catch (error) {
			if (active && !signal.aborted)
				toastStore.error(error instanceof Error ? error.message : 'Could not load file actions');
		}
	}

	return {
		paste,
		get controller() {
			return controller;
		},
		get cutPaths() {
			return cutPaths;
		},
		get favoritePaths() {
			return favoritePaths;
		},
		get canPaste() {
			return clipboardStore.hasItems || (isNativePath(location.path) && data.canCreate);
		},
		openFile(file: FileInfo) {
			if (file.isDir) location.navigate(file.path);
			else void run((actions) => actions.openFile(file));
		},
		handleContextMenuAction(action: string, items: FileInfo[]) {
			return run((actions) => actions.handleContextMenuAction(action, items));
		}
	};
}
