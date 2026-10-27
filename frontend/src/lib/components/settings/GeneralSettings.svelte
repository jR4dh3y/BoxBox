<script lang="ts">
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { Select, Toggle } from '#lib/components/ui/index.js';
	import {
		SORT_BY_OPTIONS,
		SORT_DIR_OPTIONS,
		VIEW_MODE_OPTIONS,
		pickOption
	} from '#lib/settings/options.js';
	import type { Preferences } from '#lib/settings/preferences.js';
	import SettingsRow from './SettingsRow.svelte';

	let { preferences }: { preferences: Preferences } = $props();

	const settings = $derived(settingsStore.current);
</script>

<SettingsRow label="Show hidden files">
	<Toggle
		checked={settings.showHiddenFiles}
		label="Show hidden files"
		showLabel={false}
		onchange={(showHiddenFiles) => preferences.update({ showHiddenFiles })}
	/>
</SettingsRow>
<SettingsRow label="Show file extensions">
	<Toggle
		checked={settings.showFileExtensions}
		label="Show file extensions"
		showLabel={false}
		onchange={(showFileExtensions) => preferences.update({ showFileExtensions })}
	/>
</SettingsRow>
<SettingsRow label="Confirm before delete">
	<Toggle
		checked={settings.confirmDelete}
		label="Confirm before delete"
		showLabel={false}
		onchange={(confirmDelete) => preferences.update({ confirmDelete })}
	/>
</SettingsRow>
<SettingsRow label="Preview on single click">
	<Toggle
		checked={settings.previewOnSingleClick}
		label="Preview on single click"
		showLabel={false}
		onchange={(previewOnSingleClick) => preferences.update({ previewOnSingleClick })}
	/>
</SettingsRow>
<SettingsRow label="Selection checkboxes">
	<Toggle
		checked={settings.selectionCheckboxes}
		label="Selection checkboxes"
		showLabel={false}
		onchange={(selectionCheckboxes) => preferences.update({ selectionCheckboxes })}
	/>
</SettingsRow>
<SettingsRow label="Default sort">
	<div class="w-40">
		<Select
			label="Default sort"
			options={SORT_BY_OPTIONS}
			value={settings.defaultSortBy}
			onchange={(value) => {
				const defaultSortBy = pickOption(SORT_BY_OPTIONS, value);
				if (defaultSortBy) preferences.update({ defaultSortBy });
			}}
		/>
	</div>
</SettingsRow>
<SettingsRow label="Default sort direction">
	<div class="w-40">
		<Select
			label="Default sort direction"
			options={SORT_DIR_OPTIONS}
			value={settings.defaultSortDir}
			onchange={(value) => {
				const defaultSortDir = pickOption(SORT_DIR_OPTIONS, value);
				if (defaultSortDir) preferences.update({ defaultSortDir });
			}}
		/>
	</div>
</SettingsRow>
<SettingsRow label="Default view">
	<div class="w-40">
		<Select
			label="Default view"
			options={VIEW_MODE_OPTIONS}
			value={settings.defaultViewMode}
			onchange={(value) => {
				const defaultViewMode = pickOption(VIEW_MODE_OPTIONS, value);
				if (defaultViewMode) preferences.update({ defaultViewMode });
			}}
		/>
	</div>
</SettingsRow>
