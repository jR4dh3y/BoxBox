<script lang="ts">
	import ContextMenu from '#lib/components/ui/ContextMenu.svelte';
	import {
		getFileContextMenuItems,
		type FileContextAction,
		type FileContextMenuOptions
	} from '#lib/utils/fileContextMenu.js';

	type Props = FileContextMenuOptions & {
		x: number;
		y: number;
		onSelect: (action: FileContextAction) => void;
		onClose: () => void;
	};

	let { items, canPaste, favoritePaths, canCreate, x, y, onSelect, onClose }: Props = $props();
</script>

<ContextMenu
	items={getFileContextMenuItems({ items, canPaste, favoritePaths, canCreate })}
	{x}
	{y}
	{onSelect}
	{onClose}
/>
