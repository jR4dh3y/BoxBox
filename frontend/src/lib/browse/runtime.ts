import type { BrowseLocation } from './location.svelte';
import type { BrowseData } from './data.svelte';
export interface BrowseRuntime {
	location: BrowseLocation;
	data: BrowseData;
}
