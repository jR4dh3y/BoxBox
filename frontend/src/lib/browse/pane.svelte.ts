import { useBrowseSelection } from './selection.svelte';
import { usePaneLocation, type BrowseLocationSnapshot } from './paneLocation.svelte';
export function createBrowsePaneState(
	id: string,
	initial: BrowseLocationSnapshot,
	onChange?: (snapshot: BrowseLocationSnapshot) => void
) {
	const selection = useBrowseSelection();
	const location = usePaneLocation({
		initial,
		onLocationChange: () => selection.clear(),
		onChange
	});
	return { id, selection, location };
}
export type BrowsePaneState = ReturnType<typeof createBrowsePaneState>;
