<script lang="ts">
	import { Button } from '$lib/components/ui';

	/** Shown when a window that loads on first use fails to download. A reload is the retry: browsers can cache a failed module fetch. */
	let { label, onClose }: { label: string; onClose: () => void } = $props();
</script>

<div
	role="alert"
	class="fixed top-4 left-1/2 z-[110] flex -translate-x-1/2 items-center gap-3 rounded-lg border border-border-primary bg-surface-elevated px-4 py-3 text-sm shadow-lg"
>
	<span>Could not load the {label}. Check your connection, then reload.</span>
	<Button size="sm" onclick={() => window.location.reload()}>Reload</Button>
	<Button size="sm" variant="secondary" onclick={onClose}>Close</Button>
</div>
