<script lang="ts">
	/**
	 * Settings, as a window over the file browser. Every change applies as soon as it is made.
	 */
	import { isLocalDesktop } from '#lib/api/desktop.js';
	import { browseRequests } from '#lib/browse/requests.svelte.js';
	import { onDestroy, untrack } from 'svelte';
	import { Modal } from '#lib/components/ui/index.js';
	import LoadError from '#lib/components/LoadError.svelte';
	import { toastStore } from '#lib/stores/toast.svelte.js';
	import { Preferences } from '#lib/settings/preferences.js';
	import {
		initialSettingsSection,
		settingsSections,
		type SettingsSection
	} from '#lib/settings/sections.js';
	import GeneralSettings from './GeneralSettings.svelte';
	import AppearanceSettings from './AppearanceSettings.svelte';
	import AccountSettings from './AccountSettings.svelte';

	let {
		onclose,
		initialSection = 'general'
	}: { onclose: () => void; initialSection?: SettingsSection } = $props();

	const sections = settingsSections(isLocalDesktop());

	const preferences = new Preferences();
	onDestroy(() => preferences.dispose());

	let active = $state<SettingsSection>(
		initialSettingsSection(
			sections,
			untrack(() => initialSection)
		)
	);

	async function resetPreferences() {
		if (
			!window.confirm(
				'Reset all preferences? This removes your favorites and saved local wallpaper.'
			)
		)
			return;
		const problem = await preferences.reset();
		if (problem) toastStore.error(problem, 8000);
	}
</script>

<Modal open title="Settings" size="xl" padded={false} {onclose}>
	<div class="flex h-[26rem] max-h-[calc(90vh-5.5rem)] flex-col sm:flex-row">
		<nav
			class="flex shrink-0 gap-0.5 border-b border-border-secondary p-2 sm:w-40 sm:flex-col sm:border-r sm:border-b-0"
			aria-label="Settings sections"
		>
			{#each sections as section (section.id)}
				<button
					type="button"
					class="h-8 shrink-0 grow cursor-pointer rounded px-2 text-center text-xs whitespace-nowrap transition-colors sm:grow-0 sm:px-2.5 sm:text-left sm:text-[13px] {active ===
					section.id
						? 'bg-selection text-white'
						: 'text-text-secondary hover:bg-surface-elevated hover:text-text-primary'}"
					aria-current={active === section.id ? 'true' : undefined}
					aria-label={section.label}
					onclick={() => (active = section.id)}
				>
					<span class="sm:hidden">{section.id === 'links' ? 'Links' : section.label}</span>
					<span class="hidden sm:inline">{section.label}</span>
				</button>
			{/each}
			<button
				type="button"
				class="mt-auto hidden h-8 cursor-pointer rounded px-2.5 text-left text-xs text-text-secondary transition-colors hover:bg-surface-elevated hover:text-text-primary sm:block"
				onclick={resetPreferences}
			>
				Reset to defaults
			</button>
		</nav>

		<div class="min-h-0 min-w-0 flex-1 overflow-y-auto px-3 py-2 sm:px-5">
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
			{:else if active === 'connections'}
				{#await import('../desktop/ServersPage.svelte') then { default: ServersPage }}
					<ServersPage onChanged={() => void browseRequests.changed()} />
				{:catch}<LoadError label="connections" inline />{/await}
			{:else if active === 'account'}
				<AccountSettings />
			{/if}
		</div>
		<div class="flex shrink-0 justify-end border-t border-border-secondary px-3 py-2 sm:hidden">
			<button
				type="button"
				class="cursor-pointer rounded px-2 py-1 text-xs text-text-secondary hover:bg-surface-elevated hover:text-text-primary"
				onclick={resetPreferences}>Reset to defaults</button
			>
		</div>
	</div>
</Modal>
