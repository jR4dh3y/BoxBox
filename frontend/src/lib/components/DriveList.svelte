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
	// A narrow pane drops device and filesystem rather than scrolling sideways.
	const columns =
		'grid-cols-[minmax(0,1fr)_96px_minmax(80px,0.8fr)] @sm:grid-cols-[minmax(80px,1fr)_96px_96px_minmax(90px,1fr)] @3xl:grid-cols-[minmax(140px,1fr)_minmax(140px,1fr)_104px_96px_96px_minmax(180px,1.4fr)]';
	const headers: { label: string; align: 'left' | 'right'; visibility?: 'medium' | 'wide' }[] = [
		{ label: 'Name', align: 'left' },
		{ label: 'Device', align: 'left', visibility: 'wide' },
		{ label: 'Filesystem', align: 'left', visibility: 'wide' },
		{ label: 'Size', align: 'right' },
		{ label: 'Free', align: 'right', visibility: 'medium' },
		{ label: 'Used', align: 'left' }
	];
</script>

<!-- The drives page's list view; it has the file list's header and row sizes. -->
<div class="file-surface @container h-full w-full overflow-auto bg-surface-primary">
	<div class="min-w-0">
		<div
			data-file-header="true"
			class="sticky top-0 z-[5] grid {columns} h-8 items-center border-b border-border-primary bg-surface-secondary text-[13px] font-medium whitespace-nowrap text-text-secondary select-none"
		>
			{#each headers as header (header.label)}
				<span
					class="px-3 {header.align === 'right' ? 'text-right' : ''} {header.visibility === 'wide'
						? 'hidden @3xl:block'
						: header.visibility === 'medium'
							? 'hidden @sm:block'
							: ''}">{header.label}</span
				>
			{/each}
		</div>
		{#each drives as drive (drive.name)}
			<DriveRow {drive} {columns} {compactMode} onOpen={() => onOpen?.(drive)} {onFolderHover} />
		{/each}
	</div>
</div>
