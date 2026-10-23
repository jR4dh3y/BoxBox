<script lang="ts">
	import LoadError from '#lib/components/LoadError.svelte';
</script>

{#await import('#lib/components/settings/SettingsPage.svelte')}
	<div class="flex h-full items-center justify-center text-sm text-text-secondary" role="status">
		Loading settings…
	</div>
{:then { default: SettingsPage }}
	<SettingsPage />
{:catch}
	<LoadError label="settings" />
{/await}
