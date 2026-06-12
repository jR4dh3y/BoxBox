<script lang="ts">
	/**
	 * Settings page - workspace-style preferences screen matching the file browser shell.
	 */
	import { onDestroy, tick } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { authStore } from '$lib/stores/auth';
	import {
		DEFAULT_ACCENT_COLOR,
		isValidBackgroundImage,
		isValidAccentColor,
		normalizeBackgroundImage,
		normalizeAccentColor,
		settingsStore,
		type UserSettings
	} from '$lib/stores/settings';
	import SearchBar from '$lib/components/SearchBar.svelte';
	import WallpaperSettings from '$lib/components/settings/wallpaper/WallpaperSettings.svelte';
	import { Button, ProgressButton, Select, Toggle } from '$lib/components/ui';
	import { normalizeBackgroundImageMode } from '$lib/utils/wallpaper';
	import {
		deleteLocalWallpaper,
		isInlineWallpaperDataUrl,
		isLocalWallpaperReference,
		saveLocalWallpaperDataUrl
	} from '$lib/utils/wallpaperStorage';
	import {
		ChevronLeft,
		Eye,
		Layout,
		LogOut,
		MousePointer,
		Palette,
		RotateCcw,
		Save,
		Settings,
		User,
		PaintRollerIcon,
		X
	} from 'lucide-svelte';

	type SettingsSectionId = 'display' | 'personalization' | 'behavior' | 'defaults' | 'account';
	type SettingsCategory = 'all' | SettingsSectionId;
	type ApplyProgressVariant = 'default' | 'success' | 'danger';

	let settings = $state<UserSettings>({ ...$settingsStore });
	let activeCategory = $state<SettingsCategory>('all');
	let searchQuery = $state('');
	let isApplyingSettings = $state(false);
	let applyProgress = $state(0);
	let applyProgressStatus = $state('');
	let applyProgressVariant = $state<ApplyProgressVariant>('default');
	let applyProgressResetTimer: ReturnType<typeof setTimeout> | null = null;

	const hasChanges = $derived(JSON.stringify(settings) !== JSON.stringify($settingsStore));
	const normalizedSearch = $derived(searchQuery.trim().toLowerCase());
	const accentColorIsValid = $derived(isValidAccentColor(settings.accentColor));
	const accentColorValue = $derived(
		normalizeAccentColor(settings.accentColor) ?? DEFAULT_ACCENT_COLOR
	);
	const backgroundImageIsValid = $derived(isValidBackgroundImage(settings.backgroundImage));
	const canSave = $derived(
		hasChanges && accentColorIsValid && backgroundImageIsValid && !isApplyingSettings
	);
	const showApplyProgress = $derived(isApplyingSettings || applyProgress > 0);
	const saveButtonText = $derived.by(() => {
		if (applyProgressVariant === 'danger' && applyProgress > 0) return 'Failed';
		if (applyProgressVariant === 'success' && applyProgress >= 100) return 'Saved';
		return isApplyingSettings ? 'Saving' : 'Save';
	});

	const navItems: Array<{
		id: SettingsCategory;
		label: string;
		icon: typeof Eye;
	}> = [
		{
			id: 'all',
			label: 'Show All',
			icon: Settings
		},
		{
			id: 'display',
			label: 'File Display',
			icon: Eye
		},
		{
			id: 'personalization',
			label: 'Personalization',
			icon: PaintRollerIcon
		},
		{
			id: 'behavior',
			label: 'Behavior',
			icon: MousePointer
		},
		{
			id: 'defaults',
			label: 'Default View',
			icon: Layout
		},
		{
			id: 'account',
			label: 'Account',
			icon: User
		}
	];

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

	const navButtonClass =
		'group flex w-full cursor-pointer items-center gap-2.5 border-none bg-transparent px-3 py-2 text-left transition-colors duration-100 hover:bg-surface-secondary';
	const activeNavClass = 'bg-selection text-white hover:bg-selection-hover';
	const inactiveNavClass = 'text-text-secondary hover:text-text-primary';
	const toolbarButtonClass =
		'flex h-7 w-7 cursor-pointer items-center justify-center rounded border-none bg-transparent text-text-secondary transition-all duration-100 hover:bg-surface-elevated hover:text-text-primary';
	const panelClass =
		'scroll-mt-4 overflow-hidden rounded-lg border border-border-primary bg-surface-secondary shadow-[0_18px_70px_rgba(0,0,0,0.18)]';
	const panelHeaderClass =
		'flex items-start justify-between gap-4 border-b border-border-secondary bg-surface-primary/55 px-4 py-3';
	const settingRowClass =
		'flex min-h-12 items-center justify-between gap-4 border-b border-border-secondary px-4 py-2 last:border-b-0';

	onDestroy(() => {
		clearApplyProgressResetTimer();
	});

	function clearApplyProgressResetTimer() {
		if (!applyProgressResetTimer) return;

		clearTimeout(applyProgressResetTimer);
		applyProgressResetTimer = null;
	}

	function scheduleApplyProgressReset(delay: number) {
		clearApplyProgressResetTimer();
		applyProgressResetTimer = setTimeout(() => {
			applyProgress = 0;
			applyProgressStatus = '';
			applyProgressVariant = 'default';
			applyProgressResetTimer = null;
		}, delay);
	}

	function waitForPaint(): Promise<void> {
		return new Promise((resolvePaint) => {
			if (typeof requestAnimationFrame === 'undefined') {
				resolvePaint();
				return;
			}

			requestAnimationFrame(() => resolvePaint());
		});
	}

	async function setApplyProgress(value: number, status: string) {
		applyProgress = value;
		applyProgressStatus = status;
		await tick();
		await waitForPaint();
	}

	async function handleSave() {
		if (!accentColorIsValid || !backgroundImageIsValid || isApplyingSettings) return;

		clearApplyProgressResetTimer();
		isApplyingSettings = true;
		applyProgressVariant = 'default';
		let savedLocalBackgroundImage: string | null = null;

		try {
			await setApplyProgress(15, 'Validating settings...');

			const previousBackgroundImage = $settingsStore.backgroundImage;
			let backgroundImage = normalizeBackgroundImage(settings.backgroundImage);
			if (backgroundImage && isInlineWallpaperDataUrl(backgroundImage)) {
				await setApplyProgress(35, 'Saving wallpaper locally...');
				backgroundImage = await saveLocalWallpaperDataUrl(backgroundImage);
				savedLocalBackgroundImage = backgroundImage;
			}

			const nextSettings = {
				...settings,
				accentColor: normalizeAccentColor(settings.accentColor),
				backgroundImage,
				backgroundImageMode: normalizeBackgroundImageMode(settings.backgroundImageMode),
				frostedGlass: backgroundImage ? settings.frostedGlass : false
			};

			await setApplyProgress(
				55,
				backgroundImage ? 'Applying wallpaper and preferences...' : 'Applying preferences...'
			);

			settingsStore.set(nextSettings);
			settings = { ...nextSettings };

			await setApplyProgress(85, 'Refreshing workspace...');
			cleanupLocalWallpaper(previousBackgroundImage, backgroundImage);
			applyProgressVariant = 'success';
			await setApplyProgress(100, 'Settings applied');
			scheduleApplyProgressReset(900);
		} catch (error) {
			cleanupLocalWallpaper(savedLocalBackgroundImage, null);
			applyProgressVariant = 'danger';
			applyProgress = 100;
			applyProgressStatus = getApplyErrorMessage(error);
			scheduleApplyProgressReset(4000);
		} finally {
			isApplyingSettings = false;
		}
	}

	function getApplyErrorMessage(error: unknown): string {
		if (isStorageQuotaError(error)) {
			return 'This wallpaper is too large to save locally. Choose a smaller image or pick one from the server.';
		}

		return error instanceof Error ? error.message : 'Unable to apply settings.';
	}

	function isStorageQuotaError(error: unknown): boolean {
		const errorText = error instanceof Error ? `${error.name} ${error.message}` : String(error);
		return /quota|NS_ERROR_DOM_QUOTA_REACHED|exceeded/i.test(errorText);
	}

	function cleanupLocalWallpaper(
		previousBackgroundImage: string | null,
		nextBackgroundImage: string | null
	) {
		if (
			!previousBackgroundImage ||
			previousBackgroundImage === nextBackgroundImage ||
			!isLocalWallpaperReference(previousBackgroundImage)
		) {
			return;
		}

		deleteLocalWallpaper(previousBackgroundImage).catch(() => {
			// Cleanup failure should not make an already saved preference look failed.
		});
	}

	function handleCancel() {
		clearApplyProgressResetTimer();
		applyProgress = 0;
		applyProgressStatus = '';
		applyProgressVariant = 'default';
		settings = { ...$settingsStore };
	}

	function handleReset() {
		clearApplyProgressResetTimer();
		applyProgress = 0;
		applyProgressStatus = '';
		applyProgressVariant = 'default';
		const previousBackgroundImage = $settingsStore.backgroundImage;
		settingsStore.reset();
		settings = { ...$settingsStore };
		cleanupLocalWallpaper(previousBackgroundImage, null);
	}

	async function handleLogout() {
		await authStore.logout();
		goto(resolve('/login'));
	}

	function goBack() {
		goto(resolve('/browse'));
	}

	function handleSearchInput(query: string) {
		searchQuery = query;
	}

	function handleSearchClear() {
		searchQuery = '';
	}

	function handleAccentColorInput(event: Event) {
		settings.accentColor = (event.currentTarget as HTMLInputElement).value;
	}

	function handleAccentTextInput(event: Event) {
		const value = (event.currentTarget as HTMLInputElement).value.trim();
		settings.accentColor = value === '' ? null : value;
	}

	function handleAccentReset() {
		settings.accentColor = null;
	}

	function matchesSearch(...values: string[]): boolean {
		if (!normalizedSearch) return true;
		return values.some((value) => value.toLowerCase().includes(normalizedSearch));
	}

	function categoryAllows(section: SettingsSectionId): boolean {
		return activeCategory === 'all' || activeCategory === section;
	}

	const showDisplaySection = $derived(
		categoryAllows('display') &&
			matchesSearch(
				'file display',
				'hidden files',
				'file extensions',
				'compact mode',
				'density',
				'lists',
				'grids'
			)
	);
	const showPersonalizationSection = $derived(
		categoryAllows('personalization') &&
			matchesSearch(
				'personalization',
				'accent color',
				'custom color',
				'background image',
				'wallpaper',
				'crop',
				'stretch',
				'fit',
				'wallpaper fit',
				'frosted glass',
				'blur',
				'theme',
				'folder color',
				'selection color'
			)
	);
	const showBehaviorSection = $derived(
		categoryAllows('behavior') &&
			matchesSearch(
				'behavior',
				'confirm before delete',
				'delete',
				'preview on single click',
				'preview'
			)
	);
	const showDefaultsSection = $derived(
		categoryAllows('defaults') &&
			matchesSearch('default view', 'sort by', 'sort direction', 'view mode', 'list', 'grid')
	);
	const showAccountSection = $derived(
		categoryAllows('account') &&
			matchesSearch('account', 'session', 'reset defaults', 'logout', 'local preferences')
	);
	const hasSearchResults = $derived(
		showDisplaySection ||
			showPersonalizationSection ||
			showBehaviorSection ||
			showDefaultsSection ||
			showAccountSection
	);
</script>

<svelte:head>
	<title>Settings - File Manager</title>
</svelte:head>

<div class="flex h-screen w-full overflow-hidden bg-surface-primary text-text-primary">
	<aside
		class="flex w-[220px] min-w-[220px] flex-col overflow-x-hidden overflow-y-auto border-r border-border-secondary bg-surface-primary"
	>
		<div class="border-b border-border-secondary px-3 py-3">
			<div class="flex items-center gap-2 text-[13px] font-medium text-text-primary">
				<Settings size={16} class="text-accent" />
				<span>Settings</span>
			</div>
		</div>

		<nav class="flex-1 py-2" aria-label="Settings sections">
			{#each navItems as item (item.id)}
				<button
					type="button"
					class="{navButtonClass} {activeCategory === item.id ? activeNavClass : inactiveNavClass}"
					onclick={() => (activeCategory = item.id)}
					aria-current={activeCategory === item.id ? 'page' : undefined}
				>
					<item.icon size={16} class="mt-0.5 shrink-0 opacity-80" />
					<span class="min-w-0">
						<span class="block text-[13px] leading-5">{item.label}</span>
					</span>
				</button>
			{/each}
		</nav>
	</aside>

	<div class="flex min-w-0 flex-1 flex-col">
		<div
			class="flex items-center gap-2 border-b border-border-secondary bg-surface-primary px-3 py-1.5"
		>
			<button type="button" class={toolbarButtonClass} onclick={goBack} title="Back to files">
				<ChevronLeft size={18} />
			</button>

			<div
				class="flex min-w-0 flex-1 items-center gap-1.5 rounded border border-border-primary bg-surface-secondary px-2 py-1"
			>
				<span class="text-[13px] whitespace-nowrap text-text-secondary">Settings</span>
				<span class="text-xs text-text-muted">/</span>
				<span class="text-[13px] whitespace-nowrap text-text-primary">Preferences</span>
			</div>

			<div class="w-64 shrink-0 lg:w-96">
				<SearchBar
					value={searchQuery}
					onInput={handleSearchInput}
					onClear={handleSearchClear}
					placeholder="Search settings..."
					compact
				/>
			</div>

			<div class="flex gap-1">
				<Button
					variant="ghost"
					size="sm"
					onclick={handleCancel}
					title="Discard changes"
					disabled={!hasChanges || isApplyingSettings}
				>
					<X size={16} />
					<span class="hidden sm:inline">Cancel</span>
				</Button>
				<ProgressButton
					variant="primary"
					size="sm"
					onclick={handleSave}
					title={applyProgressStatus || 'Save changes'}
					disabled={!canSave}
					busy={isApplyingSettings}
					progress={showApplyProgress ? applyProgress : null}
					progressVariant={applyProgressVariant}
					progressLabel={applyProgressStatus}
					className="min-w-20"
				>
					<Save size={16} />
					<span class="hidden sm:inline">{saveButtonText}</span>
				</ProgressButton>
			</div>
		</div>

		<main class="relative flex-1 overflow-auto">
			<div class="relative mx-auto flex max-w-[980px] flex-col gap-4 px-6 py-6">
				{#if !hasSearchResults}
					<div
						class="rounded-lg border border-border-primary bg-surface-secondary px-4 py-8 text-center"
					>
						<div class="text-sm text-text-primary">No settings found</div>
						<div class="mt-1 text-xs text-text-muted">Try a different search term.</div>
					</div>
				{/if}

				{#if showDisplaySection}
					<section id="display" class={panelClass}>
						<div class={panelHeaderClass}>
							<div>
								<h2 class="m-0 flex items-center gap-2 text-sm font-medium">
									<Eye size={16} class="text-accent" />
									File Display
								</h2>
							</div>
						</div>

						<div>
							{#if matchesSearch('show hidden files', 'display files and folders that start with a dot')}
								<div class={settingRowClass}>
									<div>
										<div class="text-[13px] text-text-primary">Show hidden files</div>
									</div>
									<Toggle
										bind:checked={settings.showHiddenFiles}
										label="Show hidden files"
										showLabel={false}
									/>
								</div>
							{/if}

							{#if matchesSearch('show file extensions', 'keep extensions visible in file names')}
								<div class={settingRowClass}>
									<div>
										<div class="text-[13px] text-text-primary">Show file extensions</div>
									</div>
									<Toggle
										bind:checked={settings.showFileExtensions}
										label="Show file extensions"
										showLabel={false}
									/>
								</div>
							{/if}

							{#if matchesSearch('compact mode', 'reduce row and tile spacing', 'density')}
								<div class={settingRowClass}>
									<div>
										<div class="text-[13px] text-text-primary">Compact mode</div>
									</div>
									<Toggle
										bind:checked={settings.compactMode}
										label="Compact mode"
										showLabel={false}
									/>
								</div>
							{/if}
						</div>
					</section>
				{/if}

				{#if showPersonalizationSection}
					<section id="personalization" class={panelClass}>
						<div class={panelHeaderClass}>
							<div>
								<h2 class="m-0 flex items-center gap-2 text-sm font-medium">
									<PaintRollerIcon size={16} class="text-accent" />
									Personalization
								</h2>
							</div>
						</div>

						<div>
							{#if matchesSearch('accent color', 'custom color', 'theme', 'folder color', 'selection color')}
								<div class={settingRowClass}>
									<div>
										<div class="flex items-center gap-2 text-[13px] text-text-primary">
											<Palette size={14} class="text-accent" />
											<span>Accent color</span>
										</div>
										{#if !accentColorIsValid}
											<div class="mt-1 text-xs text-danger">Use a #RRGGBB hex color.</div>
										{/if}
									</div>

									<div class="flex flex-wrap items-center justify-end gap-2">
										<input
											type="color"
											value={accentColorValue}
											oninput={handleAccentColorInput}
											aria-label="Choose accent color"
											class="h-8 w-10 cursor-pointer rounded border border-border-primary bg-surface-secondary p-0.5"
										/>
										<input
											type="text"
											value={settings.accentColor ?? ''}
											placeholder={DEFAULT_ACCENT_COLOR}
											oninput={handleAccentTextInput}
											aria-label="Accent color hex value"
											aria-invalid={!accentColorIsValid}
											class="h-8 w-28 rounded border bg-surface-secondary px-2 text-sm text-text-primary placeholder:text-text-muted focus:border-border-focus focus:outline-none {accentColorIsValid
												? 'border-border-primary'
												: 'border-danger'}"
										/>
										<Button variant="secondary" size="sm" onclick={handleAccentReset}
											>Default</Button
										>
									</div>
								</div>
							{/if}

							<WallpaperSettings
								rowClass={settingRowClass}
								showHiddenFiles={settings.showHiddenFiles}
								showWallpaperRow={matchesSearch(
									'background image',
									'wallpaper',
									'custom background',
									'personalization'
								)}
								showDisplayModeRow={matchesSearch(
									'wallpaper fit',
									'crop',
									'stretch',
									'fit',
									'center',
									'tile'
								)}
								showFrostedRow={matchesSearch(
									'frosted glass',
									'blur',
									'background blur',
									'glass look'
								)}
								bind:backgroundImage={settings.backgroundImage}
								bind:backgroundImageMode={settings.backgroundImageMode}
								bind:frostedGlass={settings.frostedGlass}
							/>
						</div>
					</section>
				{/if}

				{#if showBehaviorSection}
					<section id="behavior" class={panelClass}>
						<div class={panelHeaderClass}>
							<div>
								<h2 class="m-0 flex items-center gap-2 text-sm font-medium">
									<MousePointer size={16} class="text-accent" />
									Behavior
								</h2>
							</div>
						</div>

						<div>
							{#if matchesSearch('confirm before delete', 'confirmation', 'delete')}
								<div class={settingRowClass}>
									<div>
										<div class="text-[13px] text-text-primary">Confirm before delete</div>
									</div>
									<Toggle
										bind:checked={settings.confirmDelete}
										label="Confirm before delete"
										showLabel={false}
									/>
								</div>
							{/if}

							{#if matchesSearch('preview on single click', 'preview', 'single click')}
								<div class={settingRowClass}>
									<div>
										<div class="text-[13px] text-text-primary">Preview on single click</div>
									</div>
									<Toggle
										bind:checked={settings.previewOnSingleClick}
										label="Preview on single click"
										showLabel={false}
									/>
								</div>
							{/if}
						</div>
					</section>
				{/if}

				{#if showDefaultsSection}
					<section id="defaults" class={panelClass}>
						<div class={panelHeaderClass}>
							<div>
								<h2 class="m-0 flex items-center gap-2 text-sm font-medium">
									<Layout size={16} class="text-accent" />
									Default Directory View
								</h2>
							</div>
						</div>

						<div>
							{#if matchesSearch('sort by', 'default sort field', 'name', 'size', 'date modified', 'type')}
								<div class={settingRowClass}>
									<div>
										<div class="text-[13px] text-text-primary">Sort by</div>
									</div>
									<div class="w-44">
										<Select options={sortByOptions} bind:value={settings.defaultSortBy} />
									</div>
								</div>
							{/if}

							{#if matchesSearch('sort direction', 'ascending', 'descending')}
								<div class={settingRowClass}>
									<div>
										<div class="text-[13px] text-text-primary">Sort direction</div>
									</div>
									<div class="w-44">
										<Select options={sortDirOptions} bind:value={settings.defaultSortDir} />
									</div>
								</div>
							{/if}

							{#if matchesSearch('view mode', 'list', 'grid')}
								<div class={settingRowClass}>
									<div>
										<div class="text-[13px] text-text-primary">View mode</div>
									</div>
									<div class="w-44">
										<Select options={viewModeOptions} bind:value={settings.defaultViewMode} />
									</div>
								</div>
							{/if}
						</div>
					</section>
				{/if}

				{#if showAccountSection}
					<section id="account" class={panelClass}>
						<div class={panelHeaderClass}>
							<div>
								<h2 class="m-0 flex items-center gap-2 text-sm font-medium">
									<User size={16} class="text-accent" />
									Account
								</h2>
							</div>
						</div>

						<div class="grid gap-4 p-4 md:grid-cols-[1fr_auto] md:items-center">
							<div>
								<div class="text-[13px] text-text-primary">Signed in session</div>
							</div>
							<div class="flex flex-wrap gap-2">
								<Button variant="secondary" size="sm" onclick={handleReset}>
									<RotateCcw size={14} />
									Reset Defaults
								</Button>
								<Button variant="danger" size="sm" onclick={handleLogout}>
									<LogOut size={14} />
									Logout
								</Button>
							</div>
						</div>
					</section>
				{/if}
			</div>
		</main>
	</div>
</div>
