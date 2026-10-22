<script lang="ts">
	import { Network, Trash2, Usb } from 'lucide-svelte';
	import { openConnectionManager } from '#lib/api/desktop.js';
	import { desktopPanel } from '#lib/desktop/panel.svelte.js';

	let { itemClass, activeClass }: { itemClass: string; activeClass: string } = $props();
</script>

<button
	type="button"
	class="{itemClass} {desktopPanel.current === 'trash' ? activeClass : ''}"
	onclick={() => (desktopPanel.current = 'trash')}
>
	<Trash2 size={16} class="shrink-0 opacity-80" />
	<span class="flex-1 truncate">Trash</span>
</button>
<button
	type="button"
	class="{itemClass} {desktopPanel.current === 'volumes' ? activeClass : ''}"
	onclick={() => (desktopPanel.current = 'volumes')}
>
	<Usb size={16} class="shrink-0 opacity-80" />
	<span class="flex-1 truncate">Removable drives</span>
</button>
<button
	type="button"
	class={itemClass}
	title="Saved servers (Ctrl+Shift+O)"
	onclick={openConnectionManager}
>
	<Network size={16} class="shrink-0 opacity-80" />
	<span class="flex-1 truncate">Servers</span>
</button>
