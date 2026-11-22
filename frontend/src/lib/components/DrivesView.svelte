<script lang="ts">
	import type { DriveStats } from '#lib/api/files.js';
	import type { ViewMode } from '#lib/types/files.js';
	import { Spinner } from '#lib/components/ui/index.js';
	import DriveCard from './DriveCard.svelte';
	import DriveList from './DriveList.svelte';

	interface Props {
		drives: DriveStats[];
		isLoading: boolean;
		viewMode: ViewMode;
		compact?: boolean;
		onOpen: (drive: DriveStats) => void;
		onFolderHover?: (path: string | null) => void;
	}

	let { drives, isLoading, viewMode, compact = false, onOpen, onFolderHover }: Props = $props();
</script>

<!-- Overview: the drive cards, or a drive list when the list view is chosen. -->
{#if isLoading}
	<div class="flex items-center gap-2 p-6 text-sm text-text-secondary">
		<Spinner size="sm" />
		<span>Loading drives...</span>
	</div>
{:else if drives.length === 0}
	<div class="p-6 text-sm text-text-secondary">No configured storage found</div>
{:else if viewMode === 'list'}
	<DriveList {drives} {compact} {onOpen} {onFolderHover} />
{:else}
	<div class="@container h-full overflow-auto p-4 sm:p-6">
		<!-- Cap at three columns; narrower panes keep each card readable. -->
		<div class="grid grid-cols-1 gap-4 @min-[49rem]:grid-cols-2 @min-[74rem]:grid-cols-3">
			{#each drives as drive, index (drive.name)}
				<!-- The cards arrive one after another, as the desktop app's Overview cascades. -->
				<div
					class="motion-safe:animate-rise-in"
					style:animation-delay="{Math.min(index, 8) * 35}ms"
				>
					<DriveCard {drive} onClick={() => onOpen(drive)} {onFolderHover} />
				</div>
			{/each}
		</div>
	</div>
{/if}
