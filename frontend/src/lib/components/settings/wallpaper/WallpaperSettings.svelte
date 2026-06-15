<script lang="ts">
	import { Image as ImageIcon, ScanSearch, Sparkles } from 'lucide-svelte';
	import { Button, Select, Toggle } from '$lib/components/ui';
	import { isValidBackgroundImage, normalizeBackgroundImage } from '$lib/stores/settings';
	import {
		DEFAULT_BACKGROUND_IMAGE_MODE,
		WALLPAPER_DISPLAY_OPTIONS,
		normalizeBackgroundImageMode,
		type BackgroundImageMode
	} from '$lib/utils/wallpaper';
	import WallpaperPickerModal from './WallpaperPickerModal.svelte';

	interface WallpaperSelection {
		backgroundImage: string;
		mode: BackgroundImageMode;
	}

	interface Props {
		backgroundImage: string | null;
		backgroundImageMode: BackgroundImageMode;
		frostedGlass: boolean;
		showHiddenFiles: boolean;
		rowClass: string;
		showWallpaperRow?: boolean;
		showDisplayModeRow?: boolean;
		showFrostedRow?: boolean;
	}

	let {
		backgroundImage = $bindable<string | null>(null),
		backgroundImageMode = $bindable<BackgroundImageMode>(DEFAULT_BACKGROUND_IMAGE_MODE),
		frostedGlass = $bindable(false),
		showHiddenFiles,
		rowClass,
		showWallpaperRow = true,
		showDisplayModeRow = true,
		showFrostedRow = true
	}: Props = $props();

	let wallpaperDialogOpen = $state(false);

	const backgroundImageIsValid = $derived(isValidBackgroundImage(backgroundImage));
	const hasBackgroundImage = $derived(normalizeBackgroundImage(backgroundImage) !== null);
	const normalizedMode = $derived(normalizeBackgroundImageMode(backgroundImageMode));

	function handleBackgroundClear() {
		backgroundImage = null;
		frostedGlass = false;
	}

	function handleWallpaperApply(selection: WallpaperSelection) {
		backgroundImage = selection.backgroundImage;
		backgroundImageMode = selection.mode;
		wallpaperDialogOpen = false;
	}
</script>

{#if showWallpaperRow}
	<div class="{rowClass} flex-wrap">
		<div class="min-w-56">
			<div class="flex items-center gap-2 text-[13px] text-text-primary">
				<ImageIcon size={14} class="text-accent" />
				<span>Wallpaper</span>
			</div>
			{#if !backgroundImageIsValid}
				<div class="mt-1 text-xs text-danger">
					The selected wallpaper is invalid. Choose another one or clear it.
				</div>
			{/if}
		</div>

		<div class="flex min-w-64 flex-1 justify-end gap-2">
			<Button variant="secondary" size="sm" onclick={() => (wallpaperDialogOpen = true)}>
				<ImageIcon size={14} />
				Choose Wallpaper
			</Button>
			<Button
				variant="secondary"
				size="sm"
				onclick={handleBackgroundClear}
				disabled={!hasBackgroundImage}
			>
				Clear
			</Button>
		</div>
	</div>
{/if}

{#if showDisplayModeRow}
	<div class={rowClass}>
		<div>
			<div class="flex items-center gap-2 text-[13px] text-text-primary">
				<ScanSearch size={14} class="text-accent" />
				<span>Wallpaper fit</span>
			</div>
		</div>
		<div class="w-48">
			<Select
				options={WALLPAPER_DISPLAY_OPTIONS}
				bind:value={backgroundImageMode}
				disabled={!hasBackgroundImage}
			/>
		</div>
	</div>
{/if}

{#if showFrostedRow}
	<div class={rowClass}>
		<div>
			<div class="flex items-center gap-2 text-[13px] text-text-primary">
				<Sparkles size={14} class="text-accent" />
				<span>Frosted glass blur</span>
			</div>
		</div>
		<Toggle
			bind:checked={frostedGlass}
			disabled={!hasBackgroundImage}
			label="Frosted glass blur"
			showLabel={false}
		/>
	</div>
{/if}

{#if wallpaperDialogOpen}
	<WallpaperPickerModal
		open={wallpaperDialogOpen}
		currentMode={normalizedMode}
		{frostedGlass}
		{showHiddenFiles}
		onapply={handleWallpaperApply}
		onclose={() => (wallpaperDialogOpen = false)}
	/>
{/if}
