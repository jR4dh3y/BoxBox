<script lang="ts">
	import { createQueries, createQuery } from '@tanstack/svelte-query';
	import { availableRemoteServers } from '#lib/api/remotePaths.js';
	import { listRoots, getDriveStats, type MountPoint, type DriveStats } from '#lib/api/files.js';
	import { remoteCatalogKey } from '#lib/browse/remoteCatalog.js';

	let {
		atRoot,
		onchange
	}: {
		atRoot: boolean;
		onchange: (catalog: { roots: MountPoint[]; drives: DriveStats[] }) => void;
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
			queryKey: [...remoteCatalogKey, 'remote', server.id],
			queryFn: ({ signal }) => listRoots(server, signal),
			refetchInterval: 15_000
		}))
	}));
	const remoteStats = createQueries(() => ({
		queries: (serversQuery.data ?? []).map((server) => ({
			queryKey: [...remoteCatalogKey, 'stats', 'remote', server.id],
			queryFn: ({ signal }) => getDriveStats(server, signal),
			enabled: atRoot,
			refetchInterval: 15_000
		}))
	}));

	$effect(() => {
		onchange({
			roots: remoteRoots.flatMap((query) => query.data?.roots ?? []),
			drives: remoteStats.flatMap((query) => query.data?.drives ?? [])
		});
	});
</script>
