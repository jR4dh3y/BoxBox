import type { ListOptions } from '$lib/api/files';

/** Stable query keys shared by browse mutations and directory queries. */
export const fileQueryKeys = {
	all: ['files'] as const,
	roots: () => [...fileQueryKeys.all, 'roots'] as const,
	directory: (path: string) => [...fileQueryKeys.all, 'directory', path] as const,
	list: (path: string, options: Omit<ListOptions, 'page'>) =>
		[...fileQueryKeys.directory(path), options] as const,
	search: (path: string, query: string) => [...fileQueryKeys.all, 'search', path, query] as const
};
