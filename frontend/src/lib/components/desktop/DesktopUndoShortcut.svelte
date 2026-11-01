<script lang="ts">
	import { browseRequests } from '#lib/browse/requests.svelte.js';
	import { desktopUndo } from '#lib/browse/undo.svelte.js';

	function changed(): void {
		void browseRequests.changed();
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
