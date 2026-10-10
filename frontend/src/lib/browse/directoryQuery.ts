import { infiniteQueryOptions } from '@tanstack/svelte-query';
import { listDirectory, type ListOptions } from '$lib/api/files';
import { fileQueryKeys } from '$lib/stores/files';

/** One definition for the folder query, shared by the open folder and hover prefetch so they share a cache entry. */
export function directoryQueryOptions(path: string, options: Omit<ListOptions, 'page'>) {
	return infiniteQueryOptions({
		queryKey: fileQueryKeys.list(path, options),
		queryFn: ({ pageParam, signal }) =>
			listDirectory(path, { ...options, page: pageParam }, signal),
		// The cached page shows at once, but the server always confirms it.
		staleTime: 0,
		initialPageParam: 1,
		getNextPageParam: (lastPage) =>
			lastPage.page * lastPage.pageSize < lastPage.totalCount ? lastPage.page + 1 : undefined
	});
}
