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
		clear() {
			paths = new SvelteSet();
		}
	};
}

export type BrowseSelection = ReturnType<typeof useBrowseSelection>;
