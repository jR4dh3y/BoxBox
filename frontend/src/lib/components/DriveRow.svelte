<script lang="ts">
	import type { DriveStats } from '#lib/api/files.js';
	import { HardDrive } from 'lucide-svelte';
	import DeploymentIcon from './desktop/DeploymentIcon.svelte';
	import { formatFileSize } from '#lib/utils/format.js';
	import { ProgressBar } from '#lib/components/ui/index.js';
	import { DRIVE_USAGE_BAR, DRIVE_USAGE_TEXT, driveUsageLevel } from '#lib/utils/driveUsage.js';
	import LoadError from '#lib/components/LoadError.svelte';
	import { useDriveName } from '#lib/browse/driveName.svelte.js';

	interface Props {
		drive: DriveStats;
		/** The column template, shared with the list header. */
		columns: string;
		compactMode?: boolean;
		onOpen?: () => void;
		onFolderHover?: (path: string | null) => void;
	}

	let { drive, columns, compactMode = false, onOpen, onFolderHover }: Props = $props();

	const name = useDriveName(() => drive);
	const usageLevel = $derived(driveUsageLevel(drive.usedPct));
	const rowClass = $derived(
		`grid w-full ${columns} ${compactMode ? 'h-7' : 'h-8'} items-center border-b border-border-secondary text-left text-[13px]`
	);

	function handleClickOutside(event: MouseEvent): void {
		if (name.menu && event.target !== null) name.closeMenu();
	}
</script>

<svelte:window onclick={handleClickOutside} />

{#snippet cells()}
	<span
		class="truncate px-3 font-mono text-[12px] text-text-muted"
		title={drive.device || drive.serverName}
	>
		{drive.device || drive.serverName || '—'}
	</span>
	<span class="truncate px-3 font-mono text-[12px] text-text-secondary" title={drive.fsType}>
		{drive.fsType || '—'}
	</span>
	<span class="truncate px-3 text-right text-text-secondary tabular-nums">
		{formatFileSize(drive.totalBytes)}
	</span>
	<span class="truncate px-3 text-right text-text-secondary tabular-nums">
		{formatFileSize(drive.freeBytes)}
	</span>
	<span class="flex items-center gap-3 px-3">
		<span class="flex-1"
			><ProgressBar value={drive.usedPct} size="sm" variant={DRIVE_USAGE_BAR[usageLevel]} /></span
		>
		<span class="w-11 shrink-0 text-right text-[11px] font-medium {DRIVE_USAGE_TEXT[usageLevel]}">
			{drive.usedPct.toFixed(1)}%
		</span>
	</span>
{/snippet}

{#if name.renaming}
	<!-- A rename field cannot sit inside the row's button. -->
	<div class={rowClass}>
		<span class="flex min-w-0 items-center gap-2 px-3">
			{#if drive.serverIcon}<DeploymentIcon icon={drive.serverIcon} />{:else}<HardDrive
					size={16}
					class="shrink-0 text-text-secondary"
				/>{/if}
			{#await import('./ui/InlineRename.svelte') then { default: InlineRename }}
				<InlineRename value={name.displayName} onSave={name.save} onCancel={name.cancel} />
			{:catch}
				<LoadError label="rename field" onClose={name.cancel} />
			{/await}
		</span>
		{@render cells()}
	</div>
{:else}
	<button
		type="button"
		class="{rowClass} cursor-default transition-colors duration-50 hover:bg-surface-secondary"
		onclick={onOpen}
		onpointerenter={() => onFolderHover?.(drive.name)}
		onpointerleave={() => onFolderHover?.(null)}
		oncontextmenu={name.openMenu}
	>
		<span class="flex min-w-0 items-center gap-2 px-3">
			{#if drive.serverIcon}<DeploymentIcon icon={drive.serverIcon} />{:else}<HardDrive
					size={16}
					class="shrink-0 text-text-secondary"
				/>{/if}
			<span class="min-w-0 truncate text-text-primary" title={name.displayName}>
				{name.displayName}
			</span>
			{#if drive.readOnly}
				<span class="shrink-0 text-xs text-warning">Read-only</span>
			{/if}
		</span>
		{@render cells()}
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
