<script lang="ts">
	/** The toolbar's "+" menu. Loaded on first open, so it stays out of the browse page's first download. */
	import ContextMenu from '#lib/components/ui/ContextMenu.svelte';
	import { addMenuItems, toAddAction, type AddAction } from '#lib/utils/addMenu.js';

	let {
		x,
		y,
		canCreate,
		canUpload,
		onSelect,
		onClose
	}: {
		x: number;
		y: number;
		canCreate: boolean;
		canUpload: boolean;
		onSelect: (action: AddAction) => void;
		onClose: () => void;
	} = $props();

	const items = $derived(addMenuItems(canCreate, canUpload));
</script>

<ContextMenu
	{items}
	{x}
	{y}
	onSelect={(id) => {
		const action = toAddAction(id);
		if (action) onSelect(action);
	}}
	{onClose}
/>
