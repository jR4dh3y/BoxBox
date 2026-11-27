<script lang="ts">
	import { page } from '$app/state';
	import { getShareInfo } from '#lib/api/shares.js';
	import LoadError from '#lib/components/LoadError.svelte';
	import Spinner from '#lib/components/ui/Spinner.svelte';

	// Fetch the public share while its viewer downloads.
	const initialToken = page.params.token ?? '';
	const info = getShareInfo(initialToken).then(
		(value) => ({ ok: true as const, value }),
		(error: unknown) => ({ ok: false as const, error })
	);
	async function initialInfo(token: string) {
		if (token !== initialToken) return getShareInfo(token);
		const result = await info;
		if (!result.ok) throw result.error;
		return result.value;
	}
</script>

{#await import('#lib/components/RecipientShare.svelte')}
	<div class="flex h-full items-center justify-center"><Spinner /></div>
{:then { default: RecipientShare }}
	<RecipientShare {initialInfo} />
{:catch}
	<LoadError label="share" />
{/await}
