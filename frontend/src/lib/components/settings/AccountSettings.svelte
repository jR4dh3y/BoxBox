<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { authStore } from '#lib/stores/auth.svelte.js';
	import { Button } from '#lib/components/ui/index.js';
	import { currentUsername } from '#lib/utils/sessionUser.js';
	import SettingsRow from './SettingsRow.svelte';
	import PairingCodeSettings from './PairingCodeSettings.svelte';

	const username = currentUsername();

	async function signOut() {
		if (await authStore.logout()) goto(resolve('login'));
	}
</script>

{#if authStore.isDevelopment}
	<p class="m-0 py-2 text-[13px] text-text-secondary">Sign-in is turned off in development.</p>
{:else}
	<SettingsRow label={username ? `Signed in as ${username}` : 'Signed in'}>
		<Button variant="secondary" size="sm" onclick={signOut}>Sign out</Button>
	</SettingsRow>
	<PairingCodeSettings />
{/if}
