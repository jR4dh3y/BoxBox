<script lang="ts">
	import { AppWindow } from 'lucide-svelte';
	import { desktopAppIconUrl, type DesktopApplication } from '#lib/api/desktopApps.js';

	let { app }: { app: DesktopApplication } = $props();
	let failed = $state(false);
</script>

<span class="flex h-6 w-6 shrink-0 items-center justify-center text-text-muted" aria-hidden="true">
	{#if app.icon && !failed}
		<img
			src={desktopAppIconUrl(app.id)}
			alt=""
			width="24"
			height="24"
			loading="lazy"
			class="h-6 w-6 object-contain"
			onerror={() => (failed = true)}
		/>
	{:else}
		<AppWindow size={22} />
	{/if}
</span>
