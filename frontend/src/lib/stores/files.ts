import type { ListOptions } from '$lib/api/files';

/** Stable query keys shared by browse mutations and directory queries. */
export const fileQueryKeys = {
	all: ['files'] as const,
	roots: () => [...fileQueryKeys.all, 'roots'] as const,
	directories: () => [...fileQueryKeys.all, 'directory'] as const,
	directory: (path: string) => [...fileQueryKeys.directories(), path] as const,
	list: (path: string, options: Omit<ListOptions, 'page'>) =>
		[...fileQueryKeys.directory(path), options] as const,
	searches: () => [...fileQueryKeys.all, 'search'] as const,
	search: (path: string, query: string) => [...fileQueryKeys.searches(), path, query] as const
};
