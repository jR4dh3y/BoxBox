<script lang="ts">
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { toastStore } from '#lib/stores/toast.svelte.js';
	import { Button, Select, Toggle } from '#lib/components/ui/index.js';
	import LoadError from '#lib/components/LoadError.svelte';
	import {
		DEFAULT_ACCENT_COLOR,
		getBackgroundImageLabel,
		isValidAccentColor,
		normalizeAccentColor
	} from '#lib/utils/appearance.js';
	import type { AccentColorSource } from '#lib/utils/appearance.js';
	import { accentTheme } from '#lib/theme/accent.svelte.js';
	import {
		WALLPAPER_DISPLAY_OPTIONS,
		normalizeBackgroundImageMode,
		type BackgroundImageMode
	} from '#lib/utils/wallpaper.js';
	import { pickOption } from '#lib/settings/options.js';
	import type { Preferences, PreferenceProblem } from '#lib/settings/preferences.js';
	import SettingsRow from './SettingsRow.svelte';

	let { preferences }: { preferences: Preferences } = $props();
	const accentSources: Array<{ value: AccentColorSource; label: string }> = [
		{ value: 'wallpaper', label: 'From wallpaper' },
		{ value: 'manual', label: 'Custom' }
	];

	const settings = $derived(settingsStore.current);
	const wallpaperLabel = $derived(
		settings.backgroundImageName ?? getBackgroundImageLabel(settings.backgroundImage)
	);
	const hasWallpaper = $derived(settings.backgroundImage !== null);

	// The text field can hold a half-typed color; only a complete one is applied.
	let accentDraft = $state<string | null>(null);
	const accentText = $derived(accentDraft ?? settings.accentColor ?? '');
	const accentIsValid = $derived(isValidAccentColor(accentText.trim() === '' ? null : accentText));
	let pickerOpen = $state(false);
	let applying = $state(false);

	function handleAccentText(event: Event & { currentTarget: HTMLInputElement }) {
		const value = event.currentTarget.value.trim();
		accentDraft = event.currentTarget.value;
		if (value === '') preferences.setAccentColor(null);
		else if (normalizeAccentColor(value)) preferences.setAccentColor(value);
	}

	function handleAccentColor(event: Event & { currentTarget: HTMLInputElement }) {
		accentDraft = null;
		preferences.setAccentColor(event.currentTarget.value);
	}

	function resetAccent() {
		accentDraft = null;
		preferences.setAccentColor(null);
	}

	async function run(change: Promise<PreferenceProblem>) {
		applying = true;
		const problem = await change;
		applying = false;
		if (problem) toastStore.error(problem, 8000);
	}

	function chooseWallpaper(selection: {
		backgroundImage: string;
		name: string;
		mode: BackgroundImageMode;
	}) {
		pickerOpen = false;
		void run(preferences.setWallpaper(selection.backgroundImage, selection.name, selection.mode));
	}
</script>

<SettingsRow label="Accent color">
	<div class="w-40">
		<label for="accent-color-source" class="sr-only">Accent color source</label>
		<Select
			id="accent-color-source"
			options={accentSources}
			value={settings.accentColorSource}
			onchange={(value) => {
				const source = pickOption(accentSources, value);
				if (source) preferences.setAccentColorSource(source);
			}}
		/>
	</div>
</SettingsRow>
{#if settings.accentColorSource === 'wallpaper'}
	<SettingsRow label="Wallpaper color">
		<div class="flex min-w-0 items-center gap-2 text-xs text-text-secondary" role="status">
			<span class="h-4 w-4 shrink-0 rounded-full bg-accent" aria-hidden="true"></span>
			{#if !hasWallpaper}
				<span>Choose a wallpaper first</span>
			{:else if accentTheme.current.kind === 'ready'}
				<span class="font-mono">{accentTheme.current.color.toUpperCase()}</span>
			{:else if accentTheme.current.kind === 'unavailable'}
				<span
					>No usable color; using {settings.accentColor
						? 'your custom color'
						: 'default blue'}</span
				>
			{:else}
				<span>Matching wallpaper…</span>
			{/if}
		</div>
	</SettingsRow>
{:else}
	<SettingsRow label="Custom color">
		<div class="flex items-center gap-2">
			<input
				type="color"
				value={normalizeAccentColor(settings.accentColor) ?? DEFAULT_ACCENT_COLOR}
				oninput={handleAccentColor}
				aria-label="Choose accent color"
				class="h-8 w-9 cursor-pointer rounded border border-border-primary bg-surface-primary p-0.5"
			/>
			<input
				type="text"
				value={accentText}
				placeholder={DEFAULT_ACCENT_COLOR}
				oninput={handleAccentText}
				onblur={() => (accentDraft = null)}
				aria-label="Accent color hex value"
				aria-invalid={!accentIsValid}
				class="h-8 w-24 rounded border bg-surface-primary px-2 text-[13px] text-text-primary placeholder:text-text-muted focus:border-border-focus focus:outline-none {accentIsValid
					? 'border-border-primary'
					: 'border-danger'}"
			/>
			<Button variant="secondary" size="sm" onclick={resetAccent} disabled={!settings.accentColor}>
				Default
			</Button>
		</div>
	</SettingsRow>
{/if}
<SettingsRow label="Compact rows">
	<Toggle
		checked={settings.compactMode}
		label="Compact rows"
		showLabel={false}
		onchange={(compactMode) => preferences.update({ compactMode })}
	/>
</SettingsRow>
<SettingsRow label="Wallpaper">
	<div class="flex min-w-0 items-center gap-2">
		{#if wallpaperLabel}
			<span class="max-w-36 truncate text-xs text-text-secondary" title={wallpaperLabel}>
				{wallpaperLabel}
			</span>
		{/if}
		<Button variant="secondary" size="sm" onclick={() => (pickerOpen = true)} disabled={applying}>
			{hasWallpaper ? 'Change' : 'Choose'}
		</Button>
		{#if hasWallpaper}
			<Button
				variant="ghost"
				size="sm"
				onclick={() => void run(preferences.clearWallpaper())}
				disabled={applying}
			>
				Remove
			</Button>
		{/if}
	</div>
</SettingsRow>
{#if hasWallpaper}
	<SettingsRow label="Wallpaper fit">
		<div class="w-40">
			<Select
				options={WALLPAPER_DISPLAY_OPTIONS}
				value={normalizeBackgroundImageMode(settings.backgroundImageMode)}
				disabled={applying}
				onchange={(value) => {
					const mode = pickOption(WALLPAPER_DISPLAY_OPTIONS, value);
					if (mode) void run(preferences.setWallpaperMode(mode));
				}}
			/>
		</div>
	</SettingsRow>
	<SettingsRow label="Frosted glass">
		<Toggle
			checked={settings.frostedGlass}
			label="Frosted glass"
			showLabel={false}
			disabled={applying}
			onchange={(frosted) => void run(preferences.setFrostedGlass(frosted))}
		/>
	</SettingsRow>
{/if}

{#if pickerOpen}
	<!-- The picker loads on first open, so it stays out of the settings window's first download. -->
	{#await import('./wallpaper/WallpaperPickerModal.svelte') then { default: WallpaperPickerModal }}
		<WallpaperPickerModal
			open
			currentMode={normalizeBackgroundImageMode(settings.backgroundImageMode)}
			frostedGlass={settings.frostedGlass}
			showHiddenFiles={settings.showHiddenFiles}
			onapply={chooseWallpaper}
			onclose={() => (pickerOpen = false)}
		/>
	{:catch}
		<LoadError label="wallpaper picker" onClose={() => (pickerOpen = false)} />
	{/await}
{/if}
