<script lang="ts">
	import { onMount } from 'svelte';
	let loaded = $state<Awaited<typeof import('#lib/components/desktop/FileChooser.svelte')> | null>(
		null
	);
	let error = $state('');
	onMount(() => {
		void import('#lib/components/desktop/FileChooser.svelte')
			.then((module) => {
				loaded = module;
			})
			.catch(() => {
				error = 'Unable to load the file chooser. Close this window and try again.';
			});
	});
</script>

{#if loaded}<loaded.default />{:else if error}<p role="alert" class="p-6 text-danger">
		{error}
	</p>{/if}
