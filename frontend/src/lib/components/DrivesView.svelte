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
		compactMode?: boolean;
		onOpen: (drive: DriveStats) => void;
		onFolderHover?: (path: string | null) => void;
	}

	let { drives, isLoading, viewMode, compactMode = false, onOpen, onFolderHover }: Props = $props();
</script>

<!-- This Server: the drive cards, or a drive list when the list view is chosen. -->
{#if isLoading}
	<div class="flex items-center gap-2 p-6 text-sm text-text-secondary">
		<Spinner size="sm" />
		<span>Loading drives...</span>
	</div>
{:else if drives.length === 0}
	<div class="p-6 text-sm text-text-secondary">No configured storage found</div>
{:else if viewMode === 'list'}
	<DriveList {drives} {compactMode} {onOpen} {onFolderHover} />
{:else}
	<div class="h-full overflow-auto p-6">
		<div class="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
			{#each drives as drive (drive.name)}
				<DriveCard {drive} onClick={() => onOpen(drive)} {onFolderHover} />
			{/each}
		</div>
	</div>
{/if}
