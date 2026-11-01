<script lang="ts">
	import { Building2, Cloud, HardDrive, Home, Server } from 'lucide-svelte';
	import type { DeploymentIcon } from '#lib/desktop/deploymentIcon.js';
	let { icon = 'server', size = 16 }: { icon?: DeploymentIcon; size?: number } = $props();
	const icons = {
		server: Server,
		cloud: Cloud,
		home: Home,
		building: Building2,
		'hard-drive': HardDrive
	};
	const Icon = $derived(icons[icon]);
</script>

<Icon {size} />
