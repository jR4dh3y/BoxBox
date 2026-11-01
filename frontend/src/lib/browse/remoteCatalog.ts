import { QueryClient } from '@tanstack/svelte-query';
import { browseRequests } from './requests.svelte';
export const remoteCatalogKey = ['files', 'roots'];

/** Lazy, shared remote catalog. It joins domain invalidation without loading Query on startup. */
export const remoteCatalogClient = new QueryClient({
	defaultOptions: { queries: { staleTime: 60_000, retry: 1 } }
});
let pending: Promise<void> | undefined;
let revision = 0;
let epoch = 0;
browseRequests.onChange((event) => {
	if (event === 'reset') {
		epoch++;
		revision = 0;
		pending = undefined;
		remoteCatalogClient.clear();
		return;
	}
	revision++;
	if (pending) return pending;
	const session = epoch;
	const confirm = async () => {
		let observed: number;
		do {
			observed = revision;
			const alreadyFetching = remoteCatalogClient.isFetching({ queryKey: remoteCatalogKey }) > 0;
			await remoteCatalogClient.invalidateQueries(
				{ queryKey: remoteCatalogKey },
				{ cancelRefetch: false }
			);
			if (session !== epoch) return;
			if (alreadyFetching) {
				observed = revision;
				await remoteCatalogClient.invalidateQueries(
					{ queryKey: remoteCatalogKey },
					{ cancelRefetch: false }
				);
			}
		} while (session === epoch && observed !== revision);
	};
	const done = confirm().finally(() => {
		if (pending === done) pending = undefined;
	});
	pending = done;
	return done;
});
