<script lang="ts">
	/**
	 * ShareLinksSettings - every active share link, with copy and revoke.
	 */
	import { onMount } from 'svelte';
	import { Button, Spinner } from '$lib/components/ui';
	import ShareLinkList from '$lib/components/ShareLinkList.svelte';
	import { listShares, type ShareRecord } from '$lib/api';

	let shares = $state<ShareRecord[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	onMount(() => {
		void load();
	});

	async function load() {
		loading = true;
		error = null;
		try {
			shares = (await listShares()).shares;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Unable to load share links.';
		} finally {
			loading = false;
		}
	}
</script>

{#if loading}
	<div class="flex justify-center py-8"><Spinner /></div>
{:else if error}
	<div class="flex items-center justify-between gap-3 px-4 py-3">
		<p class="m-0 text-[13px] text-danger" role="alert">{error}</p>
		<Button variant="secondary" size="sm" onclick={() => void load()}>Retry</Button>
	</div>
{:else if shares.length === 0}
	<p class="m-0 px-4 py-3 text-[13px] text-text-muted">No shared links.</p>
{:else}
	<ShareLinkList
		{shares}
		showItem
		onrevoked={(share) => (shares = shares.filter((existing) => existing.id !== share.id))}
	/>
{/if}
