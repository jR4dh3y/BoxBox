<script lang="ts">
	import { Pencil, X } from 'lucide-svelte';
	import ContextMenu from '#lib/components/ui/ContextMenu.svelte';

	let {
		x,
		y,
		hasCustomName,
		onSelect,
		onClose
	}: {
		x: number;
		y: number;
		hasCustomName: boolean;
		onSelect: (id: string) => void;
		onClose: () => void;
	} = $props();

	const items = $derived([
		{ id: 'rename', label: 'Rename', icon: Pencil },
		...(hasCustomName ? [{ id: 'reset', label: 'Reset Name', icon: X }] : [])
	]);
</script>

<ContextMenu {items} {x} {y} {onSelect} {onClose} />
