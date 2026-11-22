<script lang="ts">
	import Modal from './ui/Modal.svelte';
	import { isLocalDesktop } from '#lib/api/desktop.js';
	let { onclose }: { onclose: () => void } = $props();
	// The keys `fileShortcuts.ts`, `cursor.ts` and the views bind; keep them in step.
	const groups = [
		{
			title: 'Files and selection',
			shortcuts: [
				['Arrow keys', 'Move and select'],
				['Shift + arrows', 'Extend selection'],
				['Ctrl/Cmd + arrows', 'Move without selecting'],
				['Space · Ctrl/Cmd + Space', 'Select or deselect'],
				['Home · End', 'First · last item'],
				['Page Up · Page Down', 'Move a screen'],
				['Ctrl/Cmd + click', 'Select or deselect'],
				['Drag on empty space', 'Select a rectangle; Ctrl or Shift adds'],
				['Shift · Ctrl/Cmd + Shift + click', 'Select · add a range'],
				['Type a name', 'Jump to it'],
				['Menu · Shift + F10', 'Context menu'],
				['Ctrl/Cmd + A', 'Select all'],
				['Escape', 'Clear selection or close a dialog'],
				['Enter', 'Open selected item'],
				['Ctrl/Cmd + C · X · V', 'Copy · Cut · Paste'],
				['Ctrl/Cmd + Shift + N', 'New folder'],
				['F2', 'Rename selected item'],
				['Delete', 'Trash locally; delete on a web server'],
				['Ctrl/Cmd + S', 'Select by pattern, such as *.jpg'],
				['Ctrl/Cmd + I', 'Invert selection']
			]
		},
		{
			title: 'Browse',
			shortcuts: [
				['Ctrl/Cmd + L', 'Edit location'],
				['Ctrl/Cmd + F', 'Search this folder'],
				['Alt + Left · Right', 'Back · Forward'],
				['Right-click Back · Forward', 'Places in history'],
				['Middle-click a folder', 'Open it in a new tab'],
				['Ctrl/Cmd + plus · minus · 0', 'Zoom in · out · reset'],
				['Ctrl/Cmd + wheel', 'Zoom'],
				['F11', 'Show or hide Details'],
				['F1', 'Show keyboard shortcuts']
			]
		},
		...(isLocalDesktop()
			? [
					{
						title: 'Desktop',
						shortcuts: [
							['Ctrl/Cmd + Z', 'Undo the last rename or move to Trash'],
							['Drag · Ctrl + Drag', 'Move · Copy to a folder or pane'],
							['Ctrl/Cmd + Shift + O', 'Servers']
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
