import { infiniteQueryOptions } from '@tanstack/svelte-query';
import { earlyStart, type EarlyStartSource } from '$lib/api/earlyStart';
import { directoryUrl, listDirectory, type ListOptions } from '$lib/api/files';
import { fileQueryKeys } from '$lib/stores/files';

/** One definition for the folder query, shared by the open folder and hover prefetch so they share a cache entry. */
export function directoryQueryOptions(
	path: string,
	options: Omit<ListOptions, 'page'>,
	source: EarlyStartSource = earlyStart
) {
	return infiniteQueryOptions({
		queryKey: fileQueryKeys.list(path, options),
		queryFn: async ({ pageParam, signal }) => {
			const request = { ...options, page: pageParam };
			// The shell may already have fetched the first page while the app downloaded.
			const early =
				pageParam === 1 ? await source.takeDirectory(directoryUrl(path, request)) : null;
			return early ?? listDirectory(path, request, signal);
		},
		// The cached page shows at once, but the server always confirms it.
		staleTime: 0,
		initialPageParam: 1,
		getNextPageParam: (lastPage) =>
			lastPage.page * lastPage.pageSize < lastPage.totalCount ? lastPage.page + 1 : undefined
	});
}
