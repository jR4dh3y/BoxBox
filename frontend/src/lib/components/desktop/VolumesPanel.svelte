<script lang="ts">
	import { onMount } from 'svelte';
	import { RefreshCw } from 'lucide-svelte';
	import { Button, Modal, Spinner } from '#lib/components/ui/index.js';
	import { CONFIG } from '#lib/config.js';
	import type { DesktopVolume, DesktopVolumeAction } from '#lib/api/desktopVolumes.js';
	import { VolumesController } from '#lib/desktop/volumes.svelte.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';
	import { formatFileSize } from '#lib/utils/format.js';

	interface Props {
		onClose: () => void;
		onChanged: () => void;
		onNavigate?: (path: string) => void;
	}

	let { onClose, onChanged, onNavigate }: Props = $props();

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
		if (!volume.browsePath || !onNavigate) return;
		onNavigate(volume.browsePath);
		onClose();
	}

	function label(volume: DesktopVolume, kind: DesktopVolumeAction, text: string): string {
		return action.isRunning(volume.id, kind) ? `${text}…` : text;
	}

	function space(volume: DesktopVolume): string {
		if (!volume.mounted || !volume.totalBytes) return '';
		return `${formatFileSize(volume.freeBytes ?? 0)} free of ${formatFileSize(volume.totalBytes)}`;
	}
</script>

<Modal open title="Removable drives" size="lg" onclose={onClose}>
	{#snippet headerActions()}
		<Button
			variant="ghost"
			size="icon"
			title="Refresh removable drives"
			disabled={controller.busy}
			onclick={() => void controller.load()}
		>
			<RefreshCw size={16} class={snapshot.refreshing ? 'animate-spin' : ''} />
			<span class="sr-only">Refresh removable drives</span>
		</Button>
	{/snippet}

	<div class="flex flex-col gap-3 text-[13px]" aria-busy={controller.busy}>
		{#if snapshot.state.kind === 'loading'}
			{#if showSpinner.value}
				<div class="flex items-center gap-2 py-6 text-text-secondary" role="status">
					<Spinner size="sm" />Looking for removable drives…
				</div>
			{/if}
		{:else if snapshot.state.kind === 'failed'}
			<p class="m-0 flex items-center justify-between gap-3 text-danger" role="alert">
				{snapshot.state.message}
				<Button variant="secondary" size="sm" onclick={() => void controller.load()}
					>Try again</Button
				>
			</p>
		{:else if snapshot.value && !snapshot.value.available}
			<p class="m-0 text-text-secondary">
				{snapshot.value.reason ?? 'The GIO volume monitor or UDisks is not running.'}
			</p>
		{:else if volumes.length === 0}
			<p class="m-0 py-6 text-center text-text-secondary">No removable drives connected</p>
		{:else}
			<div class="overflow-x-auto rounded border border-border-secondary">
				<table class="w-full table-fixed border-collapse" aria-label="Removable drives">
					<thead>
						<tr class="text-left text-text-secondary">
							<th class="h-8 w-[30%] bg-surface-secondary px-3 font-medium">Name</th>
							<th class="h-8 bg-surface-secondary px-3 font-medium">Device</th>
							<th class="h-8 w-40 bg-surface-secondary px-3 font-medium">Space</th>
							<th class="h-8 w-56 bg-surface-secondary px-3"
								><span class="sr-only">Actions</span></th
							>
						</tr>
					</thead>
					<tbody>
						{#each volumes as volume (volume.id)}
							<tr class="h-9 border-t border-border-secondary">
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
											{#if onNavigate && volume.browsePath}
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
			</div>
		{/if}

		{#if controller.error}
			<p class="m-0 text-danger" role="alert">{controller.error}</p>
		{:else if controller.success}
			<p class="m-0 text-success" role="status">{controller.success}</p>
		{/if}
	</div>
</Modal>
