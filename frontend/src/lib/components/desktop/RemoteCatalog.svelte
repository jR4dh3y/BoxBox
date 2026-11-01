<script lang="ts">
	import type { ComponentProps } from 'svelte';
	import { QueryClientProvider } from '@tanstack/svelte-query';
	import RemoteCatalogQueries from './RemoteCatalogQueries.svelte';
	import { remoteCatalogClient } from '#lib/browse/remoteCatalog.js';
	let props: ComponentProps<typeof RemoteCatalogQueries> = $props();
</script>

<QueryClientProvider client={remoteCatalogClient}
	><RemoteCatalogQueries {...props} /></QueryClientProvider
>
