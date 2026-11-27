<script lang="ts">
	/** The column widths and header row of a details table: sort, resize, show or hide. */
	import { Check, ChevronDown, ChevronUp } from 'lucide-svelte';
	import type { ColumnTable } from '#lib/browse/columnTable.svelte.js';
	import LoadError from './LoadError.svelte';
	import {
		AVAILABLE_COLUMNS,
		columnLabel,
		rightAligned,
		type Column
	} from '#lib/browse/columns.js';
	import type { SortDir } from '#lib/types/files.js';

	let {
		columns,
		table,
		height,
		sortedBy = null,
		sortDir = 'asc',
		onSort
	}: {
		columns: ColumnTable;
		/** The table element; borders are dragged relative to its left edge. */
		table: HTMLTableElement | null;
		height: number;
		sortedBy?: Column | null;
		sortDir?: SortDir;
		onSort?: (column: Column) => void;
	} = $props();

	let menu = $state<{ x: number; y: number } | null>(null);
	const menuItems = $derived(
		AVAILABLE_COLUMNS.map((column) => ({
			id: column,
			label: columnLabel(column),
			icon: columns.shows(column) ? Check : undefined
		}))
	);

	function chooseColumn(id: string): void {
		const column = AVAILABLE_COLUMNS.find((candidate) => candidate === id);
		if (column) columns.toggle(column);
	}

	const thClass =
		'relative text-left px-3 py-0 bg-surface-secondary border-b border-border-primary font-medium text-text-secondary whitespace-nowrap select-none sticky top-0 z-[5] transition-colors duration-100 focus:outline focus:outline-1 focus:outline-accent focus:-outline-offset-1';
</script>

<colgroup>
	{#each columns.specs as spec (spec.column)}
		<col style:width={spec.width.kind === 'fixed' ? `${spec.width.width}px` : null} />
	{/each}
</colgroup>
<thead>
	<tr
		aria-rowindex="1"
		style="height: {height}px"
		onpointerenter={() => columns.prepare()}
		oncontextmenu={(event) => {
			event.preventDefault();
			event.stopPropagation();
			menu = { x: event.clientX, y: event.clientY };
		}}
	>
		{#each columns.specs as spec, index (spec.column)}
			{@const sorted = sortedBy === spec.column}
			<th
				class="{thClass} {rightAligned(spec.column) ? 'text-right' : ''} {sorted
					? 'text-accent'
					: ''} {onSort ? 'cursor-pointer hover:bg-surface-tertiary hover:text-text-primary' : ''}"
				onclick={(event) => {
					if (event.target instanceof Element && event.target.closest('[data-column-grip]')) return;
					onSort?.(spec.column);
				}}
				onkeydown={(event) => event.key === 'Enter' && onSort?.(spec.column)}
				tabindex={onSort ? 0 : -1}
				role="columnheader"
				data-file-header="true"
				aria-sort={sorted ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
			>
				<span
					class="flex min-w-0 items-center gap-1 {rightAligned(spec.column) ? 'justify-end' : ''}"
				>
					<span class="truncate">{columnLabel(spec.column)}</span>
					<span class="inline-flex w-3 shrink-0 justify-center">
						{#if sorted}
							{#if sortDir === 'asc'}<ChevronUp size={12} />{:else}<ChevronDown size={12} />{/if}
						{/if}
					</span>
				</span>
				{#if index < columns.specs.length - 1 && table}
					{@const element = table}
					<!-- A border drag, not a control: double-click puts the default width back. -->
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<span
						class="absolute top-1 right-0 bottom-1 z-[6] w-[6px] cursor-col-resize after:absolute after:inset-y-0 after:right-0 after:w-px after:bg-border-primary hover:after:bg-accent"
						data-column-grip
						onpointerdown={(event) => columns.startResize(event, spec.column, element)}
						ondblclick={(event) => {
							event.stopPropagation();
							columns.resetWidth(spec.column);
						}}
					></span>
				{/if}
			</th>
		{/each}
	</tr>
</thead>

{#if menu}
	{#await import('./ui/ContextMenu.svelte') then { default: ContextMenu }}
		<ContextMenu
			items={menuItems}
			x={menu.x}
			y={menu.y}
			onSelect={chooseColumn}
			onClose={() => (menu = null)}
		/>
	{:catch}
		<LoadError label="column menu" onClose={() => (menu = null)} />
	{/await}
{/if}
