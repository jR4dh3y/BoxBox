<script lang="ts">
	/**
	 * Properties of one item or several: what they are and where, how much they hold with their
	 * folders walked, and the owner, group and dates the server keeps. Values can be selected.
	 */
	import { onMount } from 'svelte';
	import type { FileInfo } from '#lib/api/files.js';
	import { getProperties, type SelectionProperties } from '#lib/api/properties.js';
	import { fullFileDate, parentFolder } from '#lib/browse/details.js';
	import { formatFileSize } from '#lib/utils/format.js';
	import { getFileTypeDescription } from '#lib/utils/fileTypes.js';
	import { Modal } from '#lib/components/ui/index.js';

	let {
		items,
		locationLabel,
		onclose
	}: {
		items: readonly FileInfo[];
		/** A path as the path bar names it. */
		locationLabel: (path: string) => string;
		onclose: () => void;
	} = $props();

	type Totals =
		| { kind: 'loading' }
		| { kind: 'ready'; value: SelectionProperties }
		| { kind: 'failed'; message: string };
	let totals = $state<Totals>({ kind: 'loading' });

	onMount(() => {
		const request = new AbortController();
		getProperties(
			items.map((item) => item.path),
			request.signal
		).then(
			(value) => (totals = { kind: 'ready', value }),
			(error: unknown) => {
				if (!request.signal.aborted)
					totals = {
						kind: 'failed',
						message: error instanceof Error ? error.message : 'Could not read them'
					};
			}
		);
		return () => request.abort();
	});

	const counted = (count: number, noun: string) => `${count} ${noun}${count === 1 ? '' : 's'}`;
	const only = $derived(items.length === 1 ? items[0] : undefined);
	const parents = $derived(new Set(items.map((item) => parentFolder(item.path))));
	/** One value when every item shares it, else none. */
	function shared(pick: (index: number) => string | null): string | null {
		if (totals.kind !== 'ready') return null;
		const values = new Set(totals.value.items.map((_, index) => pick(index)));
		const [value] = values;
		return values.size === 1 && value ? value : null;
	}

	const rows = $derived.by(() => {
		const list: [string, string][] = [];
		if (only) {
			list.push(['Name', only.name]);
			list.push(['Type', only.isDir ? 'Folder' : getFileTypeDescription(only.name)]);
		} else {
			const folders = items.filter((item) => item.isDir).length;
			list.push([
				'Selected',
				`${counted(items.length - folders, 'file')}, ${counted(folders, 'folder')}`
			]);
		}
		const [parent] = parents;
		if (parents.size === 1 && parent !== undefined) list.push(['Location', locationLabel(parent)]);
		if (totals.kind === 'loading') {
			list.push([only?.isDir || !only ? 'Contains' : 'Size', 'Counting…']);
		} else if (totals.kind === 'failed') {
			list.push(['Contains', totals.message]);
		} else {
			const { files, folders, bytes, complete } = totals.value;
			const atLeast = complete ? '' : 'at least ';
			// The selected folders themselves are not inside the selection.
			const inside = folders - items.filter((item) => item.isDir).length;
			if (!only || only.isDir)
				list.push([
					'Contains',
					`${atLeast}${counted(files, 'file')}, ${counted(inside, 'folder')}`
				]);
			list.push(['Size', `${atLeast}${formatFileSize(bytes)}`]);
		}
		if (only) {
			list.push(['Modified', fullFileDate(only.modTime)]);
			const item = totals.kind === 'ready' ? totals.value.items[0] : undefined;
			if (item?.accessed) list.push(['Accessed', fullFileDate(item.accessed)]);
			if (item?.changed) list.push(['Changed', fullFileDate(item.changed)]);
		}
		const owner = shared((index) =>
			totals.kind === 'ready' ? (totals.value.items[index]?.owner ?? null) : null
		);
		const group = shared((index) =>
			totals.kind === 'ready' ? (totals.value.items[index]?.group ?? null) : null
		);
		if (owner) list.push(['Owner', owner]);
		if (group) list.push(['Group', group]);
		if (only) list.push(['Permissions', only.permissions]);
		return list;
	});
</script>

<Modal open title={only ? 'Properties' : `Properties of ${items.length} items`} {onclose}>
	<dl class="m-0 grid grid-cols-[5.5rem_minmax(0,1fr)] gap-x-4 gap-y-2.5 text-[13px]">
		{#each rows as [label, value] (label)}
			<dt class="text-text-secondary">{label}</dt>
			<dd
				class="m-0 break-all text-text-primary select-text {label === 'Permissions'
					? 'font-mono'
					: ''}"
			>
				{value}
			</dd>
		{/each}
	</dl>
</Modal>
