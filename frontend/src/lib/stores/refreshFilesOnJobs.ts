import type { QueryClient } from '@tanstack/svelte-query';
import { fileQueryKeys } from './files';
import { jobsStore } from './jobs.svelte';

/** Jobs change files after the request that started them returns, so every cached listing is suspect once one ends. */
export function refreshFilesWhenJobsFinish(queryClient: QueryClient): () => void {
	jobsStore.onFinished = () => void queryClient.invalidateQueries({ queryKey: fileQueryKeys.all });
	return () => {
		jobsStore.onFinished = null;
	};
}
