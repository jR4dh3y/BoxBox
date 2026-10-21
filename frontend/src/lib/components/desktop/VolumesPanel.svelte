<script lang="ts">
	import { onMount } from 'svelte';
	import { CircleAlert, FolderOpen, Plug, Power, RefreshCw, Unplug, Usb } from 'lucide-svelte';
	import { Badge, Button, Modal, ProgressBar, Spinner } from '#lib/components/ui/index.js';
	import { CONFIG } from '#lib/config.js';
	import { isLocalDesktop } from '#lib/api/desktop.js';
	import {
		DesktopVolumesController,
		type DesktopVolumesNotice
	} from '#lib/api/desktopVolumesController.svelte.js';
	import type { DesktopVolume, DesktopVolumeAction } from '#lib/api/desktopVolumes.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';
	import { formatFileSize } from '#lib/utils/format.js';

	interface Props {
		onClose: () => void;
		onChanged: () => void;
		onNavigate?: (path: string) => void;
	}

	let { onClose, onChanged, onNavigate }: Props = $props();

	const controller = new DesktopVolumesController();
	const localDesktop = isLocalDesktop();
	const showSpinner = createDelayedFlag(
		() => controller.loadState.kind === 'loading',
		CONFIG.ui.spinnerDelayMs
	);
	let abortController: AbortController | null = null;

	onMount(() => {
		if (!localDesktop) return;
		abortController = new AbortController();
		void controller.load(abortController.signal);
		return () => abortController?.abort();
	});

	const response = $derived(
		controller.loadState.kind === 'ready' ? controller.loadState.response : null
	);
	const volumes = $derived(response?.volumes ?? []);
	const running = $derived(
		controller.actionState.kind === 'running' ? controller.actionState : null
	);
	const busy = $derived(controller.refreshing || running !== null);

	function refresh(): void {
		if (busy) return;
		void controller.load();
	}

	function activate(volume: DesktopVolume, action: DesktopVolumeAction): void {
		void controller.act(volume, action).then((changed) => {
			if (changed) onChanged();
		});
	}

	function open(volume: DesktopVolume): void {
		if (!volume.browsePath || !onNavigate) return;
		onNavigate(volume.browsePath);
		onClose();
	}

	function isRunning(volume: DesktopVolume, action: DesktopVolumeAction): boolean {
		return running?.volumeId === volume.id && running.action === action;
	}

	function usedPercent(volume: DesktopVolume): number | null {
		if (volume.totalBytes === undefined || volume.totalBytes === 0) return null;
		const free = volume.freeBytes ?? 0;
		const used = Math.max(0, volume.totalBytes - free);
		return Math.min(100, Math.max(0, (used / volume.totalBytes) * 100));
	}

	function usageVariant(percent: number): 'default' | 'warning' | 'danger' {
		if (percent >= 90) return 'danger';
		if (percent >= 75) return 'warning';
		return 'default';
	}

	function noticeClasses(notice: DesktopVolumesNotice): string {
		return notice?.kind === 'success'
			? 'border-success/40 bg-success/10 text-success'
			: 'border-danger/40 bg-danger/10 text-danger';
	}
</script>

<Modal open={true} title="Removable drives" size="lg" onclose={onClose}>
	{#snippet headerActions()}
		<button
			type="button"
			class="rounded p-1 text-text-secondary transition-colors hover:text-text-primary disabled:opacity-40"
			title="Refresh removable drives"
			aria-label="Refresh removable drives"
			disabled={busy}
			onclick={refresh}
		>
			<RefreshCw size={16} class={controller.refreshing ? 'animate-spin' : ''} />
		</button>
	{/snippet}

	<div class="flex flex-col gap-3" aria-busy={busy}>
		{#if !localDesktop}
			<div
				class="flex items-start gap-3 rounded-md border border-border-primary bg-surface-secondary p-4"
			>
				<CircleAlert size={18} class="mt-0.5 shrink-0 text-warning" />
				<div class="min-w-0">
					<p class="text-sm font-medium text-text-primary">Desktop app only</p>
					<p class="mt-1 text-xs text-text-secondary">
						Removable drive actions are available in the BoxBox desktop app.
					</p>
				</div>
			</div>
		{:else if controller.loadState.kind === 'loading'}
			<div class="flex min-h-36 items-center justify-center gap-2 text-sm text-text-secondary">
				{#if showSpinner.value}
					<Spinner size="sm" />
				{/if}
				<span>Looking for removable drives…</span>
			</div>
		{:else if controller.loadState.kind === 'failed'}
			<div class="flex flex-col items-center gap-3 px-4 py-8 text-center">
				<CircleAlert size={24} class="text-danger" />
				<div>
					<p class="text-sm font-medium text-text-primary">Could not read removable drives</p>
					<p class="mt-1 text-xs text-text-secondary">{controller.loadState.message}</p>
				</div>
				<Button variant="secondary" size="sm" onclick={refresh}>Try again</Button>
			</div>
		{:else if response && !response.available}
			<div class="flex items-start gap-3 rounded-md border border-warning/40 bg-warning/10 p-4">
				<CircleAlert size={18} class="mt-0.5 shrink-0 text-warning" />
				<div class="min-w-0">
					<p class="text-sm font-medium text-text-primary">
						Removable drive support is unavailable
					</p>
					<p class="mt-1 text-xs text-text-secondary">
						{response.reason ?? 'The GIO volume monitor or UDisks is not running.'}
					</p>
				</div>
			</div>
		{:else if volumes.length === 0}
			<div class="flex flex-col items-center gap-2 px-4 py-10 text-center">
				<div
					class="flex h-12 w-12 items-center justify-center rounded-full bg-surface-secondary text-text-muted"
				>
					<Usb size={22} />
				</div>
				<p class="text-sm font-medium text-text-primary">No removable drives connected</p>
				<p class="max-w-sm text-xs text-text-secondary">
					Plug in a USB drive or memory card. Mounted volumes appear here, ready to open or eject.
				</p>
			</div>
		{:else}
			<ul class="flex flex-col gap-2" aria-label="Removable drives">
				{#each volumes as volume (volume.id)}
					{@const percent = usedPercent(volume)}
					<li class="rounded-lg border border-border-primary bg-surface-secondary p-3">
						<div class="flex items-start gap-3">
							<div
								class="flex h-10 w-10 shrink-0 items-center justify-center rounded bg-surface-elevated text-text-secondary"
							>
								<Usb size={20} />
							</div>
							<div class="min-w-0 flex-1">
								<div class="flex flex-wrap items-center gap-2">
									<span
										class="min-w-0 truncate text-sm font-medium text-text-primary"
										title={volume.name}
									>
										{volume.name}
									</span>
									{#if volume.readOnly}
										<Badge variant="warning">Read-only</Badge>
									{:else if !volume.mounted}
										<Badge variant="default">Not mounted</Badge>
									{/if}
								</div>
								<div
									class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-xs text-text-muted"
								>
									<span class="truncate" title={volume.driveName}>{volume.driveName}</span>
									{#if volume.device}
										<span aria-hidden="true">·</span>
										<code class="truncate font-mono text-[11px]">{volume.device}</code>
									{/if}
								</div>
								{#if volume.mountPath}
									<div class="mt-1 truncate text-xs text-text-secondary" title={volume.mountPath}>
										{volume.mountPath}
									</div>
								{/if}
								{#if volume.mounted && percent !== null}
									<div class="mt-2 max-w-sm">
										<ProgressBar value={percent} variant={usageVariant(percent)} size="sm" />
										<div class="mt-1 text-[11px] text-text-muted">
											{formatFileSize((volume.totalBytes ?? 0) - (volume.freeBytes ?? 0))} used of {formatFileSize(
												volume.totalBytes ?? 0
											)}
										</div>
									</div>
								{/if}
							</div>
							<div class="flex shrink-0 flex-wrap items-center justify-end gap-1.5">
								{#if volume.mounted}
									{#if onNavigate}
										<Button
											variant="secondary"
											size="sm"
											disabled={!volume.browsePath || busy}
											title={volume.browsePath
												? 'Open this drive'
												: 'This drive is not inside a folder BoxBox can browse'}
											onclick={() => open(volume)}
										>
											<FolderOpen size={14} />
											Open
										</Button>
									{/if}
									<Button
										variant="secondary"
										size="sm"
										disabled={!volume.canUnmount || busy}
										title={volume.canUnmount
											? 'Unmount this drive'
											: 'The system cannot unmount this drive'}
										onclick={() => activate(volume, 'unmount')}
									>
										{#if isRunning(volume, 'unmount')}
											<Spinner size="sm" />
										{:else}
											<Unplug size={14} />
										{/if}
										Unmount
									</Button>
									{#if volume.canEject}
										<Button
											variant="ghost"
											size="sm"
											disabled={busy}
											title="Safely eject this drive"
											onclick={() => activate(volume, 'eject')}
										>
											{#if isRunning(volume, 'eject')}
												<Spinner size="sm" />
											{:else}
												<Power size={14} />
											{/if}
											Eject
										</Button>
									{/if}
								{:else}
									<Button
										variant="primary"
										size="sm"
										disabled={!volume.canMount || busy}
										title={volume.canMount
											? 'Mount this drive'
											: 'The system cannot mount this drive'}
										onclick={() => activate(volume, 'mount')}
									>
										{#if isRunning(volume, 'mount')}
											<Spinner size="sm" />
										{:else}
											<Plug size={14} />
										{/if}
										Mount
									</Button>
								{/if}
							</div>
						</div>
					</li>
				{/each}
			</ul>
		{/if}

		{#if controller.notice}
			<div
				class="rounded-md border px-3 py-2 text-xs {noticeClasses(controller.notice)}"
				role={controller.notice.kind === 'error' ? 'alert' : 'status'}
				aria-live="polite"
			>
				{controller.notice.message}
			</div>
		{/if}
	</div>
</Modal>
