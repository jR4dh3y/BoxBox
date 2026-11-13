<script lang="ts">
	// Development only: the native app's components are compared against these cells.
	const dev = import.meta.env.DEV;
	import { onMount } from 'svelte';
	import {
		AppWindow,
		ClipboardPaste,
		Copy,
		ExternalLink,
		Pencil,
		RefreshCw,
		Scissors
	} from 'lucide-svelte';
	import { Button, Input, Modal, ProgressBar, Select, Toggle } from '#lib/components/ui/index.js';
	import ContextMenu from '#lib/components/ui/ContextMenu.svelte';
	import Toast from '#lib/components/ui/Toast.svelte';
	import { toastStore } from '#lib/stores/toast.svelte.js';
	import { parseGallery } from '#lib/dev/gallery.js';
	import manifest from './gallery.json';

	const gallery = parseGallery(manifest);
	const icons = {
		'app-window': AppWindow,
		'clipboard-paste': ClipboardPaste,
		copy: Copy,
		'external-link': ExternalLink,
		pencil: Pencil,
		'refresh-cw': RefreshCw,
		scissors: Scissors
	};
	const icon = (name: string | undefined) =>
		Object.entries(icons).find(([key]) => key === name)?.[1];

	onMount(() => {
		for (const cell of gallery.cells) {
			if (cell.component.kind !== 'toasts') continue;
			for (const toast of cell.component.toasts) toastStore.add(toast.type, toast.message, 0);
		}
	});
</script>

{#if dev}
	<div
		class="relative bg-surface-primary"
		style:width="{gallery.width}px"
		style:height="{gallery.height}px"
	>
		{#each gallery.cells as cell (cell.id)}
			{@const c = cell.component}
			<!-- The transform makes fixed overlays (menus, dialogs) position inside the cell. -->
			<div
				class="absolute overflow-hidden"
				style:left="{cell.x}px"
				style:top="{cell.y}px"
				style:width="{cell.w}px"
				style:height="{cell.h}px"
				style:transform="translateZ(0)"
				data-cell={cell.id}
			>
				{#if c.kind === 'button'}
					{@const Glyph = icon(c.icon)}
					<Button variant={c.variant} size={c.size} disabled={c.disabled}>
						{#if Glyph}<Glyph size={16} />{/if}
						{#if c.label}{c.label}{/if}
					</Button>
				{:else if c.kind === 'input'}
					<Input value={c.value} placeholder={c.placeholder} disabled={c.disabled} />
				{:else if c.kind === 'toggle'}
					<Toggle checked={c.checked} disabled={c.disabled} />
				{:else if c.kind === 'select'}
					<Select
						value={c.value}
						options={[{ value: c.value, label: c.value }]}
						swatch={c.swatch}
					/>
				{:else if c.kind === 'progress'}
					<ProgressBar value={c.value} size={c.size} variant={c.variant} />
				{:else if c.kind === 'context-menu'}
					<ContextMenu
						x={0}
						y={0}
						items={c.items.map((item) => ({ ...item, icon: icon(item.icon) }))}
						onSelect={() => {}}
						onClose={() => {}}
					/>
				{:else if c.kind === 'modal'}
					<Modal open title={c.title} onclose={() => {}}>
						<Input value={c.value} />
						{#snippet footer()}
							<Button variant="secondary">{c.cancel}</Button>
							<Button variant="primary">{c.confirm}</Button>
						{/snippet}
					</Modal>
				{:else if c.kind === 'toasts'}
					<div class="flex w-full flex-col gap-2">
						<Toast />
					</div>
				{/if}
			</div>
		{/each}
	</div>
{/if}
