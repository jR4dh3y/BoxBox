import { untrack } from 'svelte';
import { SvelteSet } from 'svelte/reactivity';

/** Paths the user has selected in the current folder */
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
		/** Reconcile against the complete listing, including hidden files. */
		retain(items: readonly { path: string }[]) {
			untrack(() => {
				if (paths.size === 0) return;
				// eslint-disable-next-line svelte/prefer-svelte-reactivity -- This index lives only during reconciliation.
				const existing = new Set(items.map((item) => item.path));
				const next = [...paths].filter((path) => existing.has(path));
				if (next.length !== paths.size) paths = new SvelteSet(next);
			});
		},
		clear() {
			paths = new SvelteSet();
		}
	};
}

export type BrowseSelection = ReturnType<typeof useBrowseSelection>;
