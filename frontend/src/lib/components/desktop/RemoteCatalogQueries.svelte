<script lang="ts">
	import type { RemoteCatalogSnapshot, RemoteCatalogState } from '#lib/browse/catalog.js';
	import { createQueries, createQuery } from '@tanstack/svelte-query';
	import { availableRemoteServers } from '#lib/api/remotePaths.js';
	import { listRoots, getDriveStats } from '#lib/api/files.js';
	import { remoteCatalogKey } from '#lib/browse/remoteCatalog.js';

	let {
		atRoot,
		onchange
	}: {
		atRoot: boolean;
		onchange: (catalog: RemoteCatalogSnapshot) => void;
	} = $props();
	const serversQuery = createQuery(() => ({
		queryKey: [...remoteCatalogKey, 'servers'],
		queryFn: ({ signal }) => availableRemoteServers(signal),
		staleTime: 0,
		refetchOnWindowFocus: 'always',
		refetchInterval: 15_000
	}));
	const remoteRoots = createQueries(() => ({
		queries: (serversQuery.data ?? []).map((server) => ({
			queryKey: [...remoteCatalogKey, 'remote', server.id, server.name],
			queryFn: ({ signal }) => listRoots(server, signal),
			refetchInterval: 15_000
		}))
	}));
	const remoteStats = createQueries(() => ({
		queries: (serversQuery.data ?? []).map((server) => ({
			queryKey: [...remoteCatalogKey, 'stats', 'remote', server.id, server.name],
			queryFn: ({ signal }) => getDriveStats(server, signal),
			enabled: atRoot,
			refetchInterval: 15_000
		}))
	}));

	$effect(() => {
		const states: Record<string, RemoteCatalogState> = {};
		for (const [index, server] of (serversQuery.data ?? []).entries()) {
			const roots = remoteRoots[index];
			const stats = remoteStats[index];
			const error = roots?.error ?? stats?.error;
			states[server.id] = error
				? { kind: 'error', message: error.message }
				: !roots?.isSuccess || (atRoot && !stats?.isSuccess)
					? { kind: 'loading' }
					: { kind: 'ready' };
		}
		onchange({
			status: serversQuery.isError
				? { kind: 'error', message: serversQuery.error.message }
				: serversQuery.isSuccess
					? { kind: 'ready' }
					: { kind: 'loading' },
			states,
			servers: serversQuery.data ?? [],
			roots: remoteRoots.flatMap((query) => (query.isError ? [] : (query.data?.roots ?? []))),
			drives: remoteStats.flatMap((query) => (query.isError ? [] : (query.data?.drives ?? [])))
		});
	});
</script>
