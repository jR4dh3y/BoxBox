<script lang="ts">
	import type { DriveStats } from '#lib/api/files.js';
	import DriveRow from './DriveRow.svelte';

	interface Props {
		drives: DriveStats[];
		compactMode?: boolean;
		onOpen?: (drive: DriveStats) => void;
		onFolderHover?: (path: string | null) => void;
	}

	let { drives, compactMode = false, onOpen, onFolderHover }: Props = $props();

	// Name and device share the spare width; the usage bar gets the most, since it is what the list is for.
	const columns =
		'grid-cols-[minmax(140px,1fr)_minmax(140px,1fr)_104px_96px_96px_minmax(180px,1.4fr)]';
	const headers: { label: string; align: 'left' | 'right' }[] = [
		{ label: 'Name', align: 'left' },
		{ label: 'Device', align: 'left' },
		{ label: 'Filesystem', align: 'left' },
		{ label: 'Size', align: 'right' },
		{ label: 'Free', align: 'right' },
		{ label: 'Used', align: 'left' }
	];
</script>

<!-- The drives page's list view; it has the file list's header and row sizes. -->
<div class="file-surface h-full w-full overflow-auto bg-surface-primary">
	<div class="min-w-[780px]">
		<div
			data-file-header="true"
			class="sticky top-0 z-[5] grid {columns} h-8 items-center border-b border-border-primary bg-surface-secondary text-[13px] font-medium whitespace-nowrap text-text-secondary select-none"
		>
			{#each headers as header (header.label)}
				<span class="px-3 {header.align === 'right' ? 'text-right' : ''}">{header.label}</span>
			{/each}
		</div>
		{#each drives as drive (drive.name)}
			<DriveRow {drive} {columns} {compactMode} onOpen={() => onOpen?.(drive)} {onFolderHover} />
		{/each}
	</div>
</div>
