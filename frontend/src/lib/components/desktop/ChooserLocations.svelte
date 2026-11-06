<script lang="ts">
	import { onMount } from 'svelte';
	import { Folder, HardDrive, RefreshCw, Server } from 'lucide-svelte';
	import type { FileChooser } from '#lib/desktop/chooser.svelte.js';
	import { Button } from '#lib/components/ui/index.js';
	let { chooser }: { chooser: FileChooser } = $props();
	const volumes = $derived(chooser.volumes);
	onMount(() => {
		const abort = new AbortController();
		void volumes.load(abort.signal);
		const timer = setInterval(() => {
			if (!document.hidden && !volumes.busy) void volumes.snapshot.load(abort.signal);
		}, 5000);
		return () => {
			abort.abort();
			clearInterval(timer);
		};
	});
</script>

<section aria-label="Other locations" class="min-h-0 flex-1 overflow-auto p-4">
	<div class="mb-4 flex items-center justify-between gap-3">
		<h2 class="font-medium">Other locations</h2>
		<Button
			variant="ghost"
			title="Refresh drives"
			disabled={volumes.busy}
			onclick={() => {
				void volumes.load();
				void chooser.loadServers();
			}}><RefreshCw size={16} /></Button
		>
	</div>
	<ul aria-label="Local folders" class="mb-5">
		{#each chooser.roots as root (root.name)}
			<li>
				<button
					type="button"
					class="flex w-full min-w-0 items-center gap-3 rounded px-3 py-3 text-left hover:bg-surface-secondary focus-visible:outline-2 focus-visible:outline-accent"
					onclick={() => chooser.open(root.name)}
				>
					{#if root.kind === 'drive' || root.name === 'root'}<HardDrive
							size={20}
							class="shrink-0"
						/>{:else}<Folder size={20} class="shrink-0" />{/if}
					<span class="min-w-0"
						><span class="block truncate">{root.label || root.name}</span><span
							class="block truncate text-xs text-text-secondary"
							title={root.path}>{root.path}</span
						></span
					>
				</button>
			</li>
		{/each}
	</ul>
	<h3 class="mb-2 text-text-secondary">Drives</h3>
	{#if volumes.snapshot.state.kind === 'loading'}
		<p role="status" class="px-3 py-2 text-text-secondary">Loading drives…</p>
	{:else if volumes.snapshot.state.kind === 'failed'}
		<p role="alert" class="px-3 py-2 text-danger">{volumes.snapshot.state.message}</p>
	{:else}
		{#if !volumes.snapshot.state.value.available}
			<p class="px-3 py-2 text-text-secondary">
				{volumes.snapshot.state.value.reason || 'Drive discovery is unavailable.'}
			</p>
		{:else if !volumes.snapshot.state.value.volumes.length}
			<p class="px-3 py-2 text-text-secondary">No additional drives found.</p>
		{/if}
		<ul aria-label="Drives">
			{#each volumes.snapshot.state.value.volumes as volume (volume.id)}
				<li>
					<button
						type="button"
						class="flex w-full min-w-0 items-center gap-3 rounded px-3 py-3 text-left hover:bg-surface-secondary focus-visible:outline-2 focus-visible:outline-accent disabled:opacity-50"
						disabled={volumes.busy || (!volume.browsePath && !volume.canMount)}
						onclick={() => chooser.openVolume(volume)}
						><HardDrive size={20} class="shrink-0" /><span class="min-w-0 flex-1"
							><span class="block truncate">{volume.name}</span><span
								class="block truncate text-xs text-text-secondary"
								>{volume.mountPath || volume.driveName}</span
							></span
						><span class="text-xs text-text-secondary">{volume.mounted ? 'Open' : 'Mount'}</span
						></button
					>
				</li>
			{/each}
		</ul>
	{/if}
	{#if volumes.error}<p role="alert" class="px-3 py-2 text-danger">{volumes.error}</p>{/if}
	<h3 class="mt-5 mb-2 text-text-secondary">Servers</h3>
	{#if chooser.serversError}
		<p role="alert" class="px-3 py-2 text-danger">{chooser.serversError}</p>
		<Button variant="secondary" onclick={() => chooser.loadServers()}>Retry servers</Button>
	{:else if !chooser.servers || chooser.servers.catalog.state.kind === 'loading'}
		<p role="status" class="px-3 py-2 text-text-secondary">Loading server locations…</p>
	{:else if chooser.servers.catalog.state.kind === 'failed'}
		<p role="alert" class="px-3 py-2 text-danger">{chooser.servers.catalog.state.message}</p>
		<Button variant="secondary" onclick={() => chooser.loadServers()}>Retry servers</Button>
	{:else}
		{#if !chooser.servers.catalog.state.value.length}
			<p class="px-3 py-2 text-text-secondary">
				No paired servers. Add one from Servers in BoxBox.
			</p>
		{/if}
		{#each chooser.servers.catalog.state.value as entry (entry.server.id)}
			<div class="px-3 py-2">
				<h4 class="flex items-center gap-2"><Server size={16} />{entry.server.name}</h4>
				{#if entry.roots.state.kind === 'loading'}
					<p role="status" class="mt-2 text-text-secondary">Loading locations…</p>
				{:else if entry.roots.state.kind === 'failed'}
					<p role="alert" class="mt-2 text-danger">{entry.roots.state.message}</p>
					<Button variant="secondary" onclick={() => entry.roots.load()}>Retry</Button>
				{:else}
					{#if !entry.roots.state.value.length}<p class="mt-2 text-text-secondary">
							No locations exposed by this server.
						</p>{/if}
					{#each entry.roots.state.value as root (root.name)}
						<button
							type="button"
							class="mt-1 flex w-full min-w-0 items-center gap-3 rounded px-3 py-3 text-left hover:bg-surface-secondary focus-visible:outline-2 focus-visible:outline-accent"
							onclick={() => chooser.open(root.name)}
						>
							<HardDrive size={20} class="shrink-0" /><span class="truncate"
								>{root.label || root.name}</span
							>
						</button>
					{/each}
				{/if}
			</div>
		{/each}
	{/if}
</section>
