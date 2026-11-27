<script lang="ts">
	/** A rested-on sidebar drive: how full it is and where it lives, as the desktop app's card. */
	import { isLocalDesktop } from '#lib/api/desktop.js';
	import { cardPlacement, type DriveHoverState } from '#lib/browse/hoverCard.svelte.js';
	import { ProgressBar } from '#lib/components/ui/index.js';
	import { DRIVE_USAGE_BAR, driveUsageLevel } from '#lib/utils/driveUsage.js';
	import { formatFileSize } from '#lib/utils/format.js';

	let { card }: { card: Extract<DriveHoverState, { kind: 'shown' }> } = $props();

	const WIDTH = 280;
	let height = $state(0);
	const place = $derived(
		cardPlacement(card, WIDTH, height, { width: window.innerWidth, height: window.innerHeight })
	);
	const drive = $derived(card.usage.kind === 'ready' ? card.usage.drive : null);
	const origin = $derived(
		drive?.serverName ?? (isLocalDesktop() ? 'This computer' : 'This server')
	);
	const usageKnown = $derived(
		drive !== null && drive.totalBytes > 0 && Number.isFinite(drive.usedPct)
	);
	const status = $derived(
		card.usage.kind === 'loading'
			? 'Reading…'
			: card.usage.kind === 'failed'
				? 'Could not read it'
				: 'Usage unknown'
	);
	const rows = $derived.by(() => {
		const list: [string, string][] = [];
		if (drive === null) return list;
		if (drive.mountPoint) list.push(['Location', drive.mountPoint]);
		if (drive.device) list.push(['Device', drive.device]);
		if (drive.fsType) list.push(['Filesystem', drive.fsType]);
		if (drive.readOnly) list.push(['Access', 'Read-only']);
		return list;
	});
</script>

<div
	class="hover-card pointer-events-none fixed z-[60] flex flex-col gap-2.5 rounded-lg border border-border-primary bg-surface-elevated p-3 text-xs shadow-lg"
	style:width="{WIDTH}px"
	style:left="{place.left}px"
	style:top="{place.top}px"
	bind:clientHeight={height}
	role="tooltip"
>
	<div class="flex items-baseline gap-2">
		<span class="min-w-0 flex-1 truncate text-sm text-text-primary">{card.name}</span>
		<span class="max-w-[50%] shrink-0 truncate text-text-muted">{origin}</span>
	</div>
	{#if drive !== null && usageKnown}
		<!-- The bar's track is darker here, as on the desktop's card, so it shows on the elevated card. -->
		<div class="flex flex-col gap-1.5 [&_[role=progressbar]]:bg-surface-primary">
			<ProgressBar
				value={drive.usedPct}
				size="sm"
				variant={DRIVE_USAGE_BAR[driveUsageLevel(drive.usedPct)]}
			/>
			<div class="flex justify-between gap-2">
				<span class="min-w-0 truncate text-text-primary"
					>{formatFileSize(drive.usedBytes)} / {formatFileSize(drive.totalBytes)}</span
				>
				<span class="shrink-0 text-text-secondary">{drive.usedPct.toFixed(0)}% used</span>
			</div>
		</div>
	{:else}
		<div class="flex items-start gap-2">
			<span class="w-[76px] shrink-0 text-text-muted">Status</span>
			<span class="text-text-primary">{status}</span>
		</div>
	{/if}
	{#if rows.length > 0}
		<div class="h-px bg-border-secondary"></div>
		<div class="flex flex-col gap-1">
			{#each rows as [label, value] (label)}
				<div class="flex items-start gap-2">
					<span class="w-[76px] shrink-0 text-text-muted">{label}</span>
					<!-- A path or device stays on one line, cut short, as the desktop's card. -->
					<span class="min-w-0 flex-1 truncate text-text-primary" title={value}>{value}</span>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.hover-card {
		animation: hover-card-enter 140ms cubic-bezier(0.2, 0, 0, 1);
	}
	@keyframes hover-card-enter {
		from {
			opacity: 0;
			transform: translateX(-6px);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.hover-card {
			animation: none;
		}
	}
</style>
