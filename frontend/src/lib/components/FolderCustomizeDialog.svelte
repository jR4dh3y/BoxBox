<script lang="ts">
	/** Customize Folder: a colour for the folder and its emblem, and the emblem itself. */
	import { untrack } from 'svelte';
	import { X } from 'lucide-svelte';
	import type { FileInfo } from '#lib/api/files.js';
	import { COMMON_EMBLEMS, loadEmblems, type EmblemCatalog } from '#lib/browse/emblems.js';
	import {
		DEFAULT_LOOK,
		isDefaultLook,
		lookEmblem,
		type EmblemChoice,
		type FolderLook
	} from '#lib/browse/folderLooks.js';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { Button, Input, Modal } from '#lib/components/ui/index.js';
	import EmblemShapes from './EmblemShapes.svelte';
	import FolderGlyph from './FolderGlyph.svelte';

	let {
		folder,
		own,
		onclose
	}: {
		folder: FileInfo;
		/** The special folder's own emblem, which "Default" keeps. */
		own: string | null;
		onclose: () => void;
	} = $props();

	/** The colours offered for a folder and its emblem, as the desktop app offers them. */
	const FOLDER_COLORS: readonly { color: string; label: string }[] = [
		{ color: '#ef4444', label: 'Red' },
		{ color: '#f97316', label: 'Orange' },
		{ color: '#eab308', label: 'Yellow' },
		{ color: '#22c55e', label: 'Green' },
		{ color: '#14b8a6', label: 'Teal' },
		{ color: '#06b6d4', label: 'Cyan' },
		{ color: '#3b82f6', label: 'Blue' },
		{ color: '#6366f1', label: 'Indigo' },
		{ color: '#a855f7', label: 'Purple' },
		{ color: '#ec4899', label: 'Pink' },
		{ color: '#9ca3af', label: 'Grey' },
		{ color: '#f5f5f4', label: 'White' }
	];

	const COLUMNS = 13;
	// A draft, saved on Done; the dialog closes when its folder changes.
	let look = $state<FolderLook>(
		untrack(() => settingsStore.current.folderLooks[folder.path] ?? DEFAULT_LOOK)
	);
	let query = $state('');
	const emblem = $derived(lookEmblem(look, own));

	type Choice = { kind: 'none' } | { kind: 'own'; name: string } | { kind: 'icon'; name: string };

	function choices(catalog: EmblemCatalog, search: string): Choice[] {
		if (search.trim())
			return catalog.search(search).map((name): Choice => ({ kind: 'icon', name }));
		const common = COMMON_EMBLEMS.filter((name) => catalog.shapes(name));
		const commonSet = new Set<string>(common);
		return [
			{ kind: 'none' },
			...(own ? [{ kind: 'own' as const, name: own }] : []),
			...common.map((name): Choice => ({ kind: 'icon', name })),
			...catalog.names
				.filter((name) => !commonSet.has(name))
				.map((name): Choice => ({ kind: 'icon', name }))
		];
	}

	function rows(list: Choice[]): Choice[][] {
		const result: Choice[][] = [];
		for (let index = 0; index < list.length; index += COLUMNS)
			result.push(list.slice(index, index + COLUMNS));
		return result;
	}

	function chosen(choice: Choice): boolean {
		if (choice.kind === 'none') return emblem === null;
		if (choice.kind === 'own') return look.emblem.kind === 'own';
		return look.emblem.kind === 'icon' && look.emblem.name === choice.name;
	}

	function pick(choice: Choice): void {
		const next: EmblemChoice =
			choice.kind === 'icon' ? { kind: 'icon', name: choice.name } : { kind: choice.kind };
		look = { ...look, emblem: next };
	}

	function label(choice: Choice): string {
		if (choice.kind === 'none') return 'No icon';
		return choice.kind === 'own' ? `Default (${choice.name})` : choice.name;
	}

	function done(): void {
		settingsStore.setFolderLook(folder.path, look);
		onclose();
	}

	const swatchClass =
		'h-[22px] w-[22px] shrink-0 cursor-pointer rounded-full border-2 p-[2px] focus-visible:outline focus-visible:outline-accent';
</script>

{#snippet swatch(
	color: string | null,
	selected: boolean,
	title: string,
	choose: () => void,
	fill: string
)}
	<button
		type="button"
		class="{swatchClass} {selected ? 'border-text-primary' : 'border-transparent'}"
		{title}
		aria-label={title}
		aria-pressed={selected}
		onclick={choose}
	>
		<span
			class="block h-full w-full rounded-full {color === null && fill === '' ? 'bg-folder' : ''}"
			style:background-color={fill || null}
		></span>
	</button>
{/snippet}

<Modal open size="lg" title={`Customize “${folder.name}”`} {onclose}>
	<div class="flex flex-col gap-3.5">
		<div class="flex gap-4">
			<div
				class="flex h-28 w-28 shrink-0 items-center justify-center rounded-lg bg-surface-secondary text-folder"
			>
				<FolderGlyph
					size={88}
					strokeWidth={1.2}
					color={look.color}
					{emblem}
					emblemColor={look.emblemColor}
				/>
			</div>
			<div class="flex min-w-0 flex-1 flex-col gap-3">
				<div class="flex flex-col gap-1.5">
					<span class="text-xs text-text-muted">Folder colour</span>
					<div class="flex flex-wrap gap-1">
						{@render swatch(
							null,
							look.color === null,
							'Default',
							() => (look = { ...look, color: null }),
							''
						)}
						{#each FOLDER_COLORS as { color, label } (color)}
							{@render swatch(
								color,
								look.color === color,
								label,
								() => (look = { ...look, color }),
								color
							)}
						{/each}
					</div>
				</div>
				<div class="flex flex-col gap-1.5">
					<span class="text-xs text-text-muted">Icon colour</span>
					<div class="flex flex-wrap gap-1">
						{@render swatch(
							null,
							look.emblemColor === null,
							'Same as folder',
							() => (look = { ...look, emblemColor: null }),
							look.color ?? ''
						)}
						{#each FOLDER_COLORS as { color, label } (color)}
							{@render swatch(
								color,
								look.emblemColor === color,
								label,
								() => (look = { ...look, emblemColor: color }),
								color
							)}
						{/each}
					</div>
				</div>
			</div>
		</div>

		<Input
			type="search"
			placeholder="Search icons"
			bind:value={query}
			onkeydown={(event) => {
				if (event.key === 'Enter') done();
			}}
		/>

		<div
			class="h-[248px] overflow-y-auto rounded border border-border-secondary py-1"
			role="listbox"
			aria-label="Folder icon"
		>
			{#await loadEmblems()}
				<p class="m-0 py-24 text-center text-[13px] text-text-secondary">Loading icons…</p>
			{:then catalog}
				{@const list = choices(catalog, query)}
				{#if list.length === 0}
					<p class="m-0 py-24 text-center text-[13px] text-text-secondary">No icon matches.</p>
				{:else}
					{#each rows(list) as row, rowIndex (rowIndex)}
						<!-- Rows off screen are not drawn; there are over a thousand icons. -->
						<div
							class="flex justify-center [contain-intrinsic-size:auto_40px] [content-visibility:auto]"
						>
							{#each row as choice (choice.kind === 'none' ? 'none' : `${choice.kind}:${choice.name}`)}
								{@const selected = chosen(choice)}
								{@const shapes = choice.kind === 'none' ? null : catalog.shapes(choice.name)}
								<button
									type="button"
									role="option"
									aria-selected={selected}
									class="flex h-10 w-10 shrink-0 items-center justify-center rounded-md {selected
										? 'border border-accent/60 bg-accent/20 text-accent'
										: 'text-text-secondary hover:bg-surface-secondary'}"
									title={label(choice)}
									aria-label={label(choice)}
									onclick={() => pick(choice)}
								>
									{#if shapes}<EmblemShapes {shapes} size={20} />{:else}<X
											size={18}
											class="text-text-muted"
										/>{/if}
								</button>
							{/each}
						</div>
					{/each}
				{/if}
			{:catch}
				<p class="m-0 py-24 text-center text-[13px] text-danger">Icons could not be loaded.</p>
			{/await}
		</div>
	</div>

	{#snippet footer()}
		<div class="mr-auto">
			<Button
				variant="secondary"
				disabled={isDefaultLook(look)}
				onclick={() => (look = DEFAULT_LOOK)}>Reset</Button
			>
		</div>
		<Button onclick={done}>Done</Button>
	{/snippet}
</Modal>
