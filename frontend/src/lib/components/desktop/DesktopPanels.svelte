<script lang="ts">
	import { useQueryClient } from '@tanstack/svelte-query';
	import { desktopUndo } from '#lib/browse/undo.svelte.js';
	import { desktopPanel } from '#lib/desktop/panel.svelte.js';
	import { fileQueryKeys } from '#lib/stores/files.js';

	let { onNavigate }: { onNavigate: (path: string) => void } = $props();

	const client = useQueryClient();

	function changed(): void {
		void client.invalidateQueries({ queryKey: fileQueryKeys.roots() });
		void client.invalidateQueries({ queryKey: fileQueryKeys.directories() });
	}

	function close(): void {
		desktopPanel.current = null;
	}

	function keydown(event: KeyboardEvent): void {
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

{#if desktopPanel.current === 'trash'}
	{#await import('./TrashPanel.svelte') then { default: TrashPanel }}
		<TrashPanel onChanged={changed} onClose={close} />
	{/await}
{:else if desktopPanel.current === 'volumes'}
	{#await import('./VolumesPanel.svelte') then { default: VolumesPanel }}
		<VolumesPanel onChanged={changed} {onNavigate} onClose={close} />
	{/await}
{/if}
