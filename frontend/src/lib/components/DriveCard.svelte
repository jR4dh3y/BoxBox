<script lang="ts">
	import type { DriveStats } from '#lib/api/files.js';
	import DeploymentIcon from './desktop/DeploymentIcon.svelte';
	import { HardDrive } from 'lucide-svelte';
	import { formatFileSize } from '#lib/utils/format.js';
	import { ProgressBar } from '#lib/components/ui/index.js';
	import { DRIVE_USAGE_BAR, DRIVE_USAGE_TEXT, driveUsageLevel } from '#lib/utils/driveUsage.js';
	import LoadError from '#lib/components/LoadError.svelte';
	import { useDriveName } from '#lib/browse/driveName.svelte.js';

	interface Props {
		drive: DriveStats;
		onClick?: () => void;
		/** Path of the folder under the pointer, or null once it leaves */
		onFolderHover?: (path: string | null) => void;
	}

	let { drive, onClick, onFolderHover }: Props = $props();

	const name = useDriveName(() => drive);

	const usedFormatted = $derived(formatFileSize(drive.usedBytes));
	const totalFormatted = $derived(formatFileSize(drive.totalBytes));
	const freeFormatted = $derived(formatFileSize(drive.freeBytes));

	const cardClass =
		'relative flex w-full cursor-pointer flex-wrap items-stretch gap-3 rounded-lg border border-border-primary bg-surface-secondary p-4 text-left transition-colors duration-150 hover:border-border-focus hover:bg-surface-tertiary';
	const usageLevel = $derived(driveUsageLevel(drive.usedPct));

	function handleClickOutside(e: MouseEvent): void {
		if (name.menu && e.target !== null) name.closeMenu();
	}
</script>

<svelte:window onclick={handleClickOutside} />

{#snippet contents()}
	<div
		class="flex size-22 shrink-0 items-center justify-center self-center rounded bg-surface-elevated text-text-secondary"
	>
		{#if drive.serverName}<DeploymentIcon icon={drive.serverIcon} size={22} />{:else}<HardDrive
				size={22}
			/>{/if}
	</div>

	<div class="flex min-w-0 flex-1 basis-40 flex-col gap-1.5">
		{#if name.renaming}
			<!-- Renaming is rare, so its field loads on first use. -->
			{#await import('./ui/InlineRename.svelte') then { default: InlineRename }}
				<InlineRename value={name.displayName} onSave={name.save} onCancel={name.cancel} />
			{:catch}
				<LoadError label="rename field" onClose={name.cancel} />
			{/await}
		{:else}
			<div class="flex min-w-0 flex-wrap items-start gap-x-2 gap-y-1">
				<span
					class="min-w-0 flex-1 basis-36 text-sm font-medium break-words text-text-primary"
					title={name.displayName}
				>
					{name.displayName}
				</span>
				<span
					class="inline-flex shrink-0 items-center rounded bg-surface-elevated px-2 py-0.5 text-xs font-medium whitespace-nowrap text-text-secondary"
				>
					{totalFormatted}
				</span>
			</div>
		{/if}
		<div class="text-xs text-text-muted" title="{usedFormatted} used · {freeFormatted} free">
			{usedFormatted} used · {freeFormatted} free
			{#if drive.readOnly}
				<span class="ml-1 text-warning">· Read-only</span>
			{/if}
		</div>
		{#if drive.device || drive.serverName}
			<div
				class="truncate font-mono text-[11px] text-text-muted"
				title={drive.device ? `${drive.device} (${drive.fsType || 'unknown'})` : drive.serverName}
			>
				{drive.device || drive.serverName}
				{#if drive.device && drive.fsType}
					<span class="text-text-secondary"> · {drive.fsType}</span>
				{/if}
			</div>
		{/if}

		<div class="mt-0.5 flex items-center gap-3">
			<div class="flex-1">
				<ProgressBar value={drive.usedPct} size="sm" variant={DRIVE_USAGE_BAR[usageLevel]} />
			</div>
			<span class="shrink-0 text-[11px] font-medium {DRIVE_USAGE_TEXT[usageLevel]}">
				{drive.usedPct.toFixed(1)}%
			</span>
		</div>
	</div>
{/snippet}

{#if name.renaming}
	<div class={cardClass}>{@render contents()}</div>
{:else}
	<button
		type="button"
		class={cardClass}
		onclick={onClick}
		onpointerenter={() => onFolderHover?.(drive.name)}
		onpointerleave={() => onFolderHover?.(null)}
		oncontextmenu={name.openMenu}
	>
		{@render contents()}
	</button>
{/if}

{#if name.menu && !name.renaming}
	{#await import('./DriveContextMenu.svelte') then { default: DriveContextMenu }}
		<DriveContextMenu
			x={name.menu.x}
			y={name.menu.y}
			hasCustomName={name.hasCustomName}
			onSelect={name.selectMenu}
			onClose={name.closeMenu}
		/>
	{:catch}
		<LoadError label="drive context menu" onClose={name.closeMenu} />
	{/await}
{/if}
