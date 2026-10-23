<script lang="ts">
	/**
	 * ShareLinksSettings - every active share link, with copy and revoke.
	 */
	import { isLocalDesktop } from '#lib/api/desktop.js';
	import { isRemotePath } from '#lib/api/remotePaths.js';
	import { onMount } from 'svelte';
	import { Button, Spinner } from '#lib/components/ui/index.js';
	import ShareLinkList from '#lib/components/ShareLinkList.svelte';
	import { listShares, type ShareRecord } from '#lib/api/index.js';

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
			shares = (await listShares()).shares.filter(
				(share) => !isLocalDesktop() || isRemotePath(share.id)
			);
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
	<div class="flex items-center justify-between gap-3 py-2">
		<p class="m-0 text-[13px] text-danger" role="alert">{error}</p>
		<Button variant="secondary" size="sm" onclick={() => void load()}>Retry</Button>
	</div>
{:else if shares.length === 0}
	<p class="m-0 py-2 text-[13px] text-text-secondary">No shared links.</p>
{:else}
	<ShareLinkList
		{shares}
		showItem
		onrevoked={(share) => (shares = shares.filter((existing) => existing.id !== share.id))}
	/>
{/if}
