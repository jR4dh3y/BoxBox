<script lang="ts">
	import Modal from './ui/Modal.svelte';
	import { isLocalDesktop } from '#lib/api/desktop.js';
	let { onclose }: { onclose: () => void } = $props();
	const groups = [
		{
			title: 'Files and selection',
			shortcuts: [
				['Arrow keys · Home · End', 'Move and select'],
				['Shift + Arrow keys', 'Extend selection'],
				['Ctrl/Cmd + Arrow keys', 'Move focus without selecting'],
				['Space · Ctrl/Cmd + Space', 'Select · Toggle selection'],
				['Ctrl/Cmd + A', 'Select all loaded items'],
				['Escape', 'Clear selection or close a dialog'],
				['Enter', 'Open selected item'],
				['Alt + Enter', 'Show selection details'],
				['Ctrl/Cmd + C · X · V', 'Copy · Cut · Paste'],
				['Ctrl/Cmd + Shift + N', 'New folder'],
				['F2', 'Rename selected item'],
				['Delete', 'Trash locally; delete on a web server'],
				['Shift + Delete', 'Permanently delete with confirmation']
			]
		},
		{
			title: 'Browse',
			shortcuts: [
				['Ctrl/Cmd + L', 'Edit location'],
				['Ctrl/Cmd + F', 'Search this folder'],
				['Alt + Left · Right · Up', 'Back · Forward · Parent folder'],
				['F5', 'Refresh'],
				['Ctrl/Cmd + H', 'Show or hide hidden files'],
				['Ctrl/Cmd + T · W', 'New tab · Close tab'],
				['Ctrl/Cmd + Tab', 'Next tab'],
				['Ctrl/Cmd + Shift + Tab', 'Previous tab'],
				['F3', 'Toggle split pane'],
				['F1', 'Show keyboard shortcuts']
			]
		}
	];
</script>

<Modal open={true} title="Keyboard shortcuts" size="lg" {onclose}>
	<p class="mb-4 text-sm text-text-secondary">
		File shortcuts act on the active pane. Typing fields keep their normal editing keys. Your
		browser or desktop may reserve some shortcuts.
	</p>
	<div class="grid gap-6 sm:grid-cols-2">
		{#each groups as group (group.title)}
			<section>
				<h3 class="mb-2 text-sm font-medium text-text-primary">{group.title}</h3>
				<dl class="space-y-3">
					{#each group.shortcuts as [keys, description] (keys)}
						<div>
							<dt class="text-xs font-medium text-text-primary"><kbd>{keys}</kbd></dt>
							<dd class="mt-0.5 text-xs text-text-secondary">{description}</dd>
						</div>
					{/each}
				</dl>
			</section>
		{/each}
	</div>
	{#if isLocalDesktop()}<p class="mt-5 text-xs text-text-secondary">
			Ctrl/Cmd + Z undoes the last rename or move to Trash. Ctrl + Shift + O opens saved
			connections.
		</p>{/if}
</Modal>
