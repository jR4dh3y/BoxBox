<script lang="ts">
	import { createQueries, createQuery } from '@tanstack/svelte-query';
	import { availableRemoteServers } from '#lib/api/remotePaths.js';
	import { listRoots, getDriveStats, type MountPoint, type DriveStats } from '#lib/api/files.js';
	import { fileQueryKeys } from '#lib/stores/files.js';

	let {
		atRoot,
		onchange
	}: {
		atRoot: boolean;
		onchange: (catalog: { roots: MountPoint[]; drives: DriveStats[] }) => void;
	} = $props();
	const serversQuery = createQuery(() => ({
		queryKey: [...fileQueryKeys.roots(), 'servers'],
		queryFn: availableRemoteServers,
		staleTime: 0,
		refetchOnWindowFocus: 'always',
		refetchInterval: 15_000
	}));
	const remoteRoots = createQueries(() => ({
		queries: (serversQuery.data ?? []).map((server) => ({
			queryKey: [...fileQueryKeys.roots(), 'remote', server.id],
			queryFn: () => listRoots(server),
			refetchInterval: 15_000
		}))
	}));
	const remoteStats = createQueries(() => ({
		queries: (serversQuery.data ?? []).map((server) => ({
			queryKey: [...fileQueryKeys.roots(), 'stats', 'remote', server.id],
			queryFn: () => getDriveStats(server),
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
