<script lang="ts">
	/**
	 * Settings, as a window over the file browser. Every change applies as soon as it is made.
	 */
	import { onDestroy } from 'svelte';
	import { Modal } from '#lib/components/ui/index.js';
	import LoadError from '#lib/components/LoadError.svelte';
	import { toastStore } from '#lib/stores/toast.svelte.js';
	import { Preferences } from '#lib/settings/preferences.js';
	import GeneralSettings from './GeneralSettings.svelte';
	import AppearanceSettings from './AppearanceSettings.svelte';
	import AccountSettings from './AccountSettings.svelte';

	let { onclose }: { onclose: () => void } = $props();

	type Section = 'general' | 'appearance' | 'links' | 'account';
	const sections: { id: Section; label: string }[] = [
		{ id: 'general', label: 'General' },
		{ id: 'appearance', label: 'Appearance' },
		{ id: 'links', label: 'Shared links' },
		{ id: 'account', label: 'Account' }
	];

	const preferences = new Preferences();
	onDestroy(() => preferences.dispose());

	let active = $state<Section>('general');

	async function resetPreferences() {
		const problem = await preferences.reset();
		if (problem) toastStore.error(problem, 8000);
	}
</script>

<Modal open title="Settings" size="xl" padded={false} {onclose}>
	<div class="flex h-[26rem] max-h-[calc(90vh-5.5rem)]">
		<nav
			class="flex w-28 shrink-0 flex-col gap-0.5 border-r border-border-secondary p-2 sm:w-40"
			aria-label="Settings sections"
		>
			{#each sections as section (section.id)}
				<button
					type="button"
					class="h-8 cursor-pointer rounded px-2.5 text-left text-[13px] transition-colors {active ===
					section.id
						? 'bg-selection text-white'
						: 'text-text-secondary hover:bg-surface-elevated hover:text-text-primary'}"
					aria-current={active === section.id ? 'true' : undefined}
					onclick={() => (active = section.id)}
				>
					{section.label}
				</button>
			{/each}
			<button
				type="button"
				class="mt-auto h-8 cursor-pointer rounded px-2.5 text-left text-xs text-text-secondary transition-colors hover:bg-surface-elevated hover:text-text-primary"
				onclick={resetPreferences}
			>
				Reset to defaults
			</button>
		</nav>

		<div class="min-w-0 flex-1 overflow-y-auto px-3 py-2 sm:px-5">
			{#if active === 'general'}
				<GeneralSettings {preferences} />
			{:else if active === 'appearance'}
				<AppearanceSettings {preferences} />
			{:else if active === 'links'}
				{#await import('./ShareLinksSettings.svelte') then { default: ShareLinksSettings }}
					<ShareLinksSettings />
				{:catch}
					<LoadError label="shared links" inline />
				{/await}
			{:else}
				<AccountSettings />
			{/if}
		</div>
	</div>
</Modal>
