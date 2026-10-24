<script lang="ts">
	import LoadError from '#lib/components/LoadError.svelte';
</script>

{#await import('#lib/components/settings/SettingsPage.svelte') then { default: SettingsPage }}
	<SettingsPage />
{:catch}
	<LoadError label="settings" />
{/await}
