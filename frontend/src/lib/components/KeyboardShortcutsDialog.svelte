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
		},
		...(isLocalDesktop()
			? [
					{
						title: 'Desktop',
						shortcuts: [
							['Ctrl/Cmd + Z', 'Undo the last rename or move to Trash'],
							['Ctrl + Shift + O', 'Saved connections']
						]
					}
				]
			: [])
	];
</script>

<Modal open title="Keyboard shortcuts" size="lg" {onclose}>
	<div class="grid gap-6 text-xs sm:grid-cols-2">
		{#each groups as group (group.title)}
			<table class="w-full border-collapse self-start">
				<caption class="pb-1.5 text-left text-[13px] font-medium text-text-primary"
					>{group.title}</caption
				>
				<tbody>
					{#each group.shortcuts as [keys, description] (keys)}
						<tr class="border-t border-border-secondary">
							<th scope="row" class="py-1.5 pr-3 text-left font-normal whitespace-nowrap"
								><kbd class="font-sans text-text-primary">{keys}</kbd></th
							>
							<td class="py-1.5 text-text-secondary">{description}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/each}
	</div>
</Modal>
