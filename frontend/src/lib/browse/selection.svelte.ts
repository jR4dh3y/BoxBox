import { untrack } from 'svelte';
import { SvelteSet } from 'svelte/reactivity';

/** Paths selected in the current folder or its search results. */
export function useBrowseSelection() {
	let paths = $state.raw<Set<string>>(new SvelteSet());

	return {
		get paths() {
			return paths;
		},
		get count() {
			return paths.size;
		},
		set(next: Set<string>) {
			paths = next;
		},
		/** Prune missing children only in the confirmed folder, including hidden files. */
		retain(items: readonly { path: string }[], folder: string) {
			untrack(() => {
				if (paths.size === 0) return;
				// eslint-disable-next-line svelte/prefer-svelte-reactivity -- This index lives only during reconciliation.
				const existing = new Set(items.map((item) => item.path));
				const next = [...paths].filter(
					(path) =>
						existing.has(path) || path.slice(0, Math.max(0, path.lastIndexOf('/'))) !== folder
				);
				if (next.length !== paths.size) paths = new SvelteSet(next);
			});
		},
		clear() {
			paths = new SvelteSet();
		}
	};
}

export type BrowseSelection = ReturnType<typeof useBrowseSelection>;
