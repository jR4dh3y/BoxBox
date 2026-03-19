<script lang="ts">
	/**
	 * Settings page - user preferences and account management
	 * Design follows the same visual language as the file browser sidebar
	 */
	import { goto } from '$app/navigation';
	import { authStore } from '$lib/stores/auth.svelte';
	import { settingsStore, type UserSettings } from '$lib/stores/settings.svelte';
	import {
		ChevronLeft,
		Eye,
		RotateCcw,
		Layout,
		MousePointer,
		User,
		LogOut,
		Save,
		X
	} from 'lucide-svelte';
	import { Button, Toggle, Select } from '$lib/components/ui';
	import { SettingsSection, SettingsRow } from '$lib/components/settings';

	function cloneSettings(source: UserSettings): UserSettings {
		return {
			...source,
			driveNameOverrides: { ...source.driveNameOverrides }
		};
	}

	let settings = $state<UserSettings>(cloneSettings(settingsStore.value));
	const hasChanges = $derived(JSON.stringify(settings) !== JSON.stringify(settingsStore.value));

	// Section collapse state
	let displayCollapsed = $state(false);
	let behaviorCollapsed = $state(false);
	let viewCollapsed = $state(false);
	let accountCollapsed = $state(false);

	function handleSave() {
		settingsStore.set(settings);
	}

	function handleReset() {
		settingsStore.reset();
		settings = cloneSettings(settingsStore.value);
	}

	function handleCancel() {
		settings = cloneSettings(settingsStore.value);
	}

	async function handleLogout() {
		await authStore.logout();
		goto('/login');
	}

	function goBack() {
		goto('/browse');
	}

	const sortByOptions = [
		{ value: 'name', label: 'Name' },
		{ value: 'size', label: 'Size' },
		{ value: 'modTime', label: 'Date modified' },
		{ value: 'type', label: 'Type' }
	];

	const sortDirOptions = [
		{ value: 'asc', label: 'Ascending' },
		{ value: 'desc', label: 'Descending' }
	];

	const viewModeOptions = [
		{ value: 'list', label: 'List' },
		{ value: 'grid', label: 'Grid' }
	];

	// Shared styles for navigation items
	const navItemClass =
		'w-full flex items-center gap-2.5 py-1.5 px-3 pl-5 bg-transparent border-none text-[13px] cursor-pointer text-left transition-colors duration-100 hover:bg-surface-secondary';
	const backButtonClass =
		'w-7 h-7 flex items-center justify-center bg-transparent border-none rounded text-text-secondary cursor-pointer transition-all duration-100 hover:bg-surface-elevated hover:text-text-primary';
</script>

<svelte:head>
	<title>Settings - File Manager</title>
</svelte:head>

<div class="flex h-screen w-full overflow-hidden bg-surface-primary">
	<!-- Settings Sidebar (matching main sidebar width) -->
	<aside
		class="flex w-[220px] min-w-[220px] flex-col overflow-x-hidden overflow-y-auto border-r border-border-secondary bg-surface-primary"
	>
		<!-- Header -->
		<div class="flex items-center gap-2 border-b border-border-secondary px-3 py-3">
			<button type="button" class={backButtonClass} onclick={goBack} title="Back to Files">
				<ChevronLeft size={18} />
			</button>
			<span class="text-[13px] font-medium text-text-primary">Settings</span>
		</div>

		<!-- Navigation within settings -->
		<nav class="flex-1 py-2">
			<button type="button" class="{navItemClass} bg-selection text-text-primary">
				<Eye size={16} class="shrink-0 opacity-80" />
				<span>Preferences</span>
			</button>
			<button type="button" class="{navItemClass} text-text-secondary" onclick={handleLogout}>
				<LogOut size={16} class="shrink-0 opacity-80" />
				<span>Logout</span>
			</button>
		</nav>
	</aside>

	<!-- Main content area -->
	<div class="flex min-w-0 flex-1 flex-col">
		<!-- Toolbar matching browse toolbar -->
		<div
			class="flex items-center gap-2 border-b border-border-secondary bg-surface-primary px-3 py-1.5"
		>
			<div class="flex items-center gap-2">
				<button type="button" class={backButtonClass} onclick={goBack} title="Back">
					<ChevronLeft size={18} />
				</button>
			</div>

			<!-- Path bar style breadcrumb -->
			<div
				class="flex min-w-0 flex-1 items-center gap-1.5 rounded border border-border-primary bg-surface-secondary px-2 py-1"
			>
				<span class="text-[13px] text-text-secondary">Settings</span>
				<span class="text-xs text-text-muted">/</span>
				<span class="text-[13px] text-text-primary">Preferences</span>
			</div>

			<!-- Action buttons -->
			{#if hasChanges}
				<div class="flex gap-1">
					<Button variant="ghost" size="sm" onclick={handleCancel} title="Discard changes">
						<X size={16} />
						<span class="hidden sm:inline">Cancel</span>
					</Button>
					<Button variant="primary" size="sm" onclick={handleSave} title="Save changes">
						<Save size={16} />
						<span class="hidden sm:inline">Save</span>
					</Button>
				</div>
			{/if}
		</div>

		<!-- Settings content (styled like file list area) -->
		<div class="flex-1 overflow-auto">
			<div class="max-w-[600px]">
				<!-- File Display Section -->
				<SettingsSection title="File Display" icon={Eye} bind:collapsed={displayCollapsed}>
					<SettingsRow label="Show hidden files" description="Display files starting with a dot">
						<Toggle bind:checked={settings.showHiddenFiles} />
					</SettingsRow>

					<SettingsRow label="Show file extensions" description="Display extensions in file list">
						<Toggle bind:checked={settings.showFileExtensions} />
					</SettingsRow>

					<SettingsRow label="Compact mode" description="Reduce spacing for more items">
						<Toggle bind:checked={settings.compactMode} />
					</SettingsRow>
				</SettingsSection>

				<!-- Behavior Section -->
				<SettingsSection title="Behavior" icon={MousePointer} bind:collapsed={behaviorCollapsed}>
					<SettingsRow label="Confirm before delete" description="Show confirmation dialog">
						<Toggle bind:checked={settings.confirmDelete} />
					</SettingsRow>

					<SettingsRow label="Preview on single click" description="Open preview with single click">
						<Toggle bind:checked={settings.previewOnSingleClick} />
					</SettingsRow>
				</SettingsSection>

				<!-- Default View Section -->
				<SettingsSection title="Default View" icon={Layout} bind:collapsed={viewCollapsed}>
					<SettingsRow label="Sort by" description="Default sort field">
						<div class="w-32">
							<Select options={sortByOptions} bind:value={settings.defaultSortBy} />
						</div>
					</SettingsRow>

					<SettingsRow label="Sort direction" description="Ascending or descending">
						<div class="w-32">
							<Select options={sortDirOptions} bind:value={settings.defaultSortDir} />
						</div>
					</SettingsRow>

					<SettingsRow label="View mode" description="List or grid view">
						<div class="w-32">
							<Select options={viewModeOptions} bind:value={settings.defaultViewMode} />
						</div>
					</SettingsRow>
				</SettingsSection>

				<!-- Account Section -->
				<SettingsSection title="Account" icon={User} bind:collapsed={accountCollapsed}>
					<div class="px-3">
						<div class="flex gap-2 pt-2">
							<Button variant="secondary" size="sm" onclick={handleReset}>
								<RotateCcw size={14} />
								Reset to Defaults
							</Button>
							<Button variant="danger" size="sm" onclick={handleLogout}>
								<LogOut size={14} />
								Logout
							</Button>
						</div>
					</div>
				</SettingsSection>
			</div>
		</div>

		<!-- Status bar matching browse status bar -->
		<div
			class="flex items-center justify-between border-t border-border-secondary bg-surface-primary px-3 py-1.5 text-[11px] text-text-muted"
		>
			<span>
				{#if hasChanges}
					<span class="text-warning">● Unsaved changes</span>
				{:else}
					All changes saved
				{/if}
			</span>
			<span>Preferences</span>
		</div>
	</div>
</div>
