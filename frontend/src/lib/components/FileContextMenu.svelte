<script lang="ts">
	import ContextMenu from '#lib/components/ui/ContextMenu.svelte';
	import {
		getFileContextMenuItems,
		type FileContextMenuOptions
	} from '#lib/utils/fileContextMenu.js';

	interface Props extends Pick<
		FileContextMenuOptions,
		'items' | 'canPaste' | 'favoritePaths' | 'canCreate'
	> {
		x: number;
		y: number;
		onSelect: (action: string) => void;
		onClose: () => void;
	}

	let { items, canPaste, favoritePaths, canCreate, x, y, onSelect, onClose }: Props = $props();

	const menuItems = $derived(
		getFileContextMenuItems({ items, canPaste, favoritePaths, canCreate })
	);
</script>

<ContextMenu items={menuItems} {x} {y} {onSelect} {onClose} />
