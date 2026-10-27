import { queryOptions } from '@tanstack/svelte-query';
import { earlyStart, type EarlyStartSource } from '#lib/api/earlyStart.js';
import { directoryUrl, listDirectory } from '#lib/api/files.js';
import { fileQueryKeys } from '#lib/stores/files.js';

/**
 * One definition for the folder query, shared by the open folder and hover prefetch. The key is the
 * path alone: sorting and hidden files are applied locally, so changing them needs no request.
 */
export function directoryQueryOptions(path: string, source: EarlyStartSource = earlyStart) {
	return queryOptions({
		queryKey: fileQueryKeys.directory(path),
		queryFn: async ({ signal }) => {
			// The shell may already have fetched this listing while the app downloaded.
			const early = await source.takeDirectory(directoryUrl(path));
			return early ?? listDirectory(path, signal);
		},
		// The cached listing shows at once, but the server always confirms it.
		staleTime: 0
	});
}
