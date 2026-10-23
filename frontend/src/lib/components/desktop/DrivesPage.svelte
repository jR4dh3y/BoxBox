<script lang="ts">
	import { onMount } from 'svelte';
	import { RefreshCw } from 'lucide-svelte';
	import { Button, Spinner } from '#lib/components/ui/index.js';
	import { CONFIG } from '#lib/config.js';
	import type { DesktopVolume, DesktopVolumeAction } from '#lib/api/desktopVolumes.js';
	import { VolumesController } from '#lib/desktop/volumes.svelte.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';
	import { formatFileSize } from '#lib/utils/format.js';

	interface Props {
		onChanged: () => void;
		onNavigate: (path: string) => void;
	}

	let { onChanged, onNavigate }: Props = $props();

	const controller = new VolumesController();
	const { snapshot, action } = controller;
	const showSpinner = createDelayedFlag(
		() => snapshot.state.kind === 'loading',
		CONFIG.ui.spinnerDelayMs
	);
	const volumes = $derived(snapshot.value?.volumes ?? []);

	onMount(() => {
		const request = new AbortController();
		void controller.load(request.signal);
		return () => request.abort();
	});

	function activate(volume: DesktopVolume, kind: DesktopVolumeAction): void {
		void controller.act(volume, kind).then((changed) => {
			if (changed) onChanged();
		});
	}

	function open(volume: DesktopVolume): void {
		if (volume.browsePath) onNavigate(volume.browsePath);
	}

	function label(volume: DesktopVolume, kind: DesktopVolumeAction, text: string): string {
		return action.isRunning(volume.id, kind) ? `${text}…` : text;
	}

	function space(volume: DesktopVolume): string {
		if (!volume.mounted || !volume.totalBytes) return '';
		return `${formatFileSize(volume.freeBytes ?? 0)} free of ${formatFileSize(volume.totalBytes)}`;
	}
</script>

<div class="flex h-full min-h-0 flex-col text-[13px]" aria-busy={controller.busy}>
	<div class="flex h-10 shrink-0 items-center gap-2 border-b border-border-secondary px-3">
		<p class="m-0 min-w-0 flex-1 truncate">
			{#if controller.error}
				<span class="text-danger" role="alert">{controller.error}</span>
			{:else if controller.success}
				<span class="text-success" role="status">{controller.success}</span>
			{/if}
		</p>
		<Button
			variant="ghost"
			size="icon"
			title="Look for removable drives"
			disabled={controller.busy}
			onclick={() => void controller.load()}
		>
			<RefreshCw size={16} class={snapshot.refreshing ? 'animate-spin' : ''} />
			<span class="sr-only">Look for removable drives</span>
		</Button>
	</div>

	<div class="min-h-0 flex-1 overflow-auto">
		{#if snapshot.state.kind === 'loading'}
			{#if showSpinner.value}
				<div class="flex items-center justify-center gap-2 py-10 text-text-secondary" role="status">
					<Spinner size="sm" />Looking for removable drives…
				</div>
			{/if}
		{:else if snapshot.state.kind === 'failed'}
			<p class="m-0 flex items-center justify-center gap-3 py-10 text-danger" role="alert">
				{snapshot.state.message}
				<Button variant="secondary" size="sm" onclick={() => void controller.load()}
					>Try again</Button
				>
			</p>
		{:else if snapshot.value && !snapshot.value.available}
			<p class="m-0 py-12 text-center text-text-secondary">
				{snapshot.value.reason ?? 'The GIO volume monitor or UDisks is not running.'}
			</p>
		{:else if volumes.length === 0}
			<p class="m-0 py-12 text-center text-text-muted">No removable drives connected</p>
		{:else}
			<table class="w-full table-fixed border-collapse" aria-label="Removable drives">
				<thead>
					<tr class="text-left text-text-secondary">
						<th
							class="sticky top-0 h-8 w-[30%] border-b border-border-primary bg-surface-secondary px-3 font-medium"
							>Name</th
						>
						<th
							class="sticky top-0 h-8 border-b border-border-primary bg-surface-secondary px-3 font-medium"
							>Device</th
						>
						<th
							class="sticky top-0 h-8 w-44 border-b border-border-primary bg-surface-secondary px-3 font-medium"
							>Space</th
						>
						<th
							class="sticky top-0 h-8 w-56 border-b border-border-primary bg-surface-secondary px-3"
							><span class="sr-only">Actions</span></th
						>
					</tr>
				</thead>
				<tbody>
					{#each volumes as volume (volume.id)}
						<tr class="h-9 border-b border-border-secondary hover:bg-surface-secondary">
							<td class="truncate px-3 text-text-primary" title={volume.driveName}>
								{volume.name}{#if volume.readOnly}<span class="text-text-muted">
										· read-only</span
									>{/if}
							</td>
							<td class="truncate px-3 font-mono text-xs text-text-secondary"
								>{volume.device ?? ''}</td
							>
							<td class="truncate px-3 text-text-secondary">{space(volume)}</td>
							<td class="px-3">
								<div class="flex justify-end gap-1.5">
									{#if volume.mounted}
										{#if volume.browsePath}
											<Button
												variant="secondary"
												size="sm"
												disabled={controller.busy}
												onclick={() => open(volume)}>Open</Button
											>
										{/if}
										{#if volume.canUnmount}
											<Button
												variant="secondary"
												size="sm"
												disabled={controller.busy}
												onclick={() => activate(volume, 'unmount')}
												>{label(volume, 'unmount', 'Unmount')}</Button
											>
										{/if}
										{#if volume.canEject}
											<Button
												variant="secondary"
												size="sm"
												disabled={controller.busy}
												onclick={() => activate(volume, 'eject')}
												>{label(volume, 'eject', 'Eject')}</Button
											>
										{/if}
									{:else if volume.canMount}
										<Button
											variant="primary"
											size="sm"
											disabled={controller.busy}
											onclick={() => activate(volume, 'mount')}
											>{label(volume, 'mount', 'Mount')}</Button
										>
									{/if}
								</div>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>
