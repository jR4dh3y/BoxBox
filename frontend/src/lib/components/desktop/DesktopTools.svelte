<script lang="ts">
	import { HardDrive, Trash2, Network, Terminal, Eye, EyeOff, Undo2 } from 'lucide-svelte';
	import { desktopUndo } from '#lib/browse/undo.svelte.js';
	import { openDesktopTerminal } from '#lib/api/desktopApps.js';
	import { useQueryClient } from '@tanstack/svelte-query';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { toastStore } from '#lib/stores/toast.svelte.js';
	import { fileQueryKeys } from '#lib/stores/files.js';
	import { openConnectionManager } from '#lib/api/desktop.js';
	let { path, onNavigate }: { path: string; onNavigate?: (path: string) => void } = $props();
	let panel = $state<'trash' | 'volumes' | null>(null);
	const client = useQueryClient();
	const buttonClass =
		'inline-flex items-center gap-1.5 rounded px-2 py-1 text-xs text-text-secondary hover:bg-surface-secondary hover:text-text-primary focus-visible:outline-2 focus-visible:outline-accent disabled:opacity-40';
	function changed() {
		void client.invalidateQueries({ queryKey: fileQueryKeys.roots() });
		void client.invalidateQueries({ queryKey: fileQueryKeys.directories() });
	}
	async function terminal() {
		try {
			await openDesktopTerminal(path);
		} catch (error) {
			toastStore.error(error instanceof Error ? error.message : 'Open terminal failed');
		}
	}
	function keydown(event: KeyboardEvent) {
		if (
			event.defaultPrevented ||
			event.shiftKey ||
			event.altKey ||
			!(event.ctrlKey || event.metaKey) ||
			event.key.toLowerCase() !== 'z'
		)
			return;
		if (
			event.target instanceof HTMLElement &&
			event.target.closest('input, textarea, [contenteditable="true"], [role="dialog"]')
		)
			return;
		if (desktopUndo.available) {
			event.preventDefault();
			void desktopUndo.run(changed);
		}
	}
</script>

<svelte:window onkeydown={keydown} />

<div
	class="flex flex-wrap items-center gap-1 border-b border-border-secondary bg-surface-primary px-3 py-1"
	aria-label="Desktop tools"
>
	<button type="button" class={buttonClass} onclick={() => (panel = 'trash')}
		><Trash2 size={14} />Trash</button
	>
	<button type="button" class={buttonClass} onclick={() => (panel = 'volumes')}
		><HardDrive size={14} />Drives</button
	>
	<button
		type="button"
		class={buttonClass}
		onclick={openConnectionManager}
		title="Saved connections (Ctrl+Shift+O)"><Network size={14} />Connections</button
	>
	<button type="button" class={buttonClass} onclick={terminal} disabled={!path}
		><Terminal size={14} />Terminal</button
	>
	<button
		type="button"
		class={buttonClass}
		onclick={() => settingsStore.toggleHiddenFiles()}
		aria-pressed={settingsStore.current.showHiddenFiles}
	>
		{#if settingsStore.current.showHiddenFiles}<EyeOff size={14} />{:else}<Eye
				size={14}
			/>{/if}Hidden files
	</button>
	<button
		type="button"
		class={buttonClass}
		onclick={() => desktopUndo.run(changed)}
		disabled={!desktopUndo.available}
		title="Undo the last rename or move to Trash (Ctrl+Z)"><Undo2 size={14} />Undo</button
	>
</div>

{#if panel === 'trash'}
	{#await import('./TrashPanel.svelte') then { default: TrashPanel }}
		<TrashPanel onChanged={changed} onClose={() => (panel = null)} />
	{/await}
{:else if panel === 'volumes'}
	{#await import('./VolumesPanel.svelte') then { default: VolumesPanel }}
		<VolumesPanel onChanged={changed} {onNavigate} onClose={() => (panel = null)} />
	{/await}
{/if}
