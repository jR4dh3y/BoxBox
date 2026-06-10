<script lang="ts">
	/**
	 * Settings page - workspace-style preferences screen matching the file browser shell.
	 */
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { authStore } from '$lib/stores/auth';
	import {
		DEFAULT_ACCENT_COLOR,
		isValidBackgroundImage,
		isValidAccentColor,
		normalizeBackgroundImage,
		normalizeAccentColor,
		resolveBackgroundImage,
		settingsStore,
		toServerBackgroundImage,
		type UserSettings
	} from '$lib/stores/settings';
	import SearchBar from '$lib/components/SearchBar.svelte';
	import { Button, Modal, Select, Toggle } from '$lib/components/ui';
	import { listDirectory, listRoots, type FileInfo, type MountPoint } from '$lib/api/files';
	import { formatFileSize } from '$lib/utils/format';
	import { getPreviewType } from '$lib/utils/fileTypes';
	import {
		AlertTriangle,
		ChevronUp,
		ChevronLeft,
		Eye,
		FileImage,
		Folder,
		HardDrive,
		Image as ImageIcon,
		Layout,
		LogOut,
		MousePointer,
		Palette,
		RotateCcw,
		Save,
		Settings,
		Sparkles,
		Upload,
		User,
		PaintRollerIcon,
		X
	} from 'lucide-svelte';

	type SettingsSectionId = 'display' | 'personalization' | 'behavior' | 'defaults' | 'account';
	type SettingsCategory = 'all' | SettingsSectionId;
	type WallpaperSource = 'server';

	const LOCAL_WALLPAPER_MAX_BYTES = 10 * 1024 * 1024;

	let settings = $state<UserSettings>({ ...$settingsStore });
	let activeCategory = $state<SettingsCategory>('all');
	let searchQuery = $state('');
	let wallpaperDialogOpen = $state(false);
	let wallpaperSource = $state<WallpaperSource | null>(null);
	let serverWallpaperRoots = $state<MountPoint[]>([]);
	let serverWallpaperPath = $state('');
	let serverWallpaperItems = $state<FileInfo[]>([]);
	let serverWallpaperLoading = $state(false);
	let serverWallpaperError = $state<string | null>(null);
	let localWallpaperError = $state<string | null>(null);
	let localWallpaperInput: HTMLInputElement;

	const hasChanges = $derived(JSON.stringify(settings) !== JSON.stringify($settingsStore));
	const normalizedSearch = $derived(searchQuery.trim().toLowerCase());
	const accentColorIsValid = $derived(isValidAccentColor(settings.accentColor));
	const accentColorValue = $derived(
		normalizeAccentColor(settings.accentColor) ?? DEFAULT_ACCENT_COLOR
	);
	const backgroundImageIsValid = $derived(isValidBackgroundImage(settings.backgroundImage));
	const normalizedBackgroundImage = $derived(resolveBackgroundImage(settings.backgroundImage));
	const hasBackgroundImage = $derived(normalizedBackgroundImage !== null);
	const serverWallpaperEntries = $derived.by(() =>
		serverWallpaperItems.filter((item) => item.isDir || isImageFile(item))
	);
	const serverWallpaperCrumbs = $derived(
		serverWallpaperPath ? serverWallpaperPath.split('/').filter(Boolean) : []
	);
	const canSave = $derived(hasChanges && accentColorIsValid && backgroundImageIsValid);

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
		'flex items-center justify-between gap-4 border-b border-border-secondary px-4 py-3 last:border-b-0';

	function handleSave() {
		if (!accentColorIsValid || !backgroundImageIsValid) return;

		const backgroundImage = normalizeBackgroundImage(settings.backgroundImage);

		settingsStore.set({
			...settings,
			accentColor: normalizeAccentColor(settings.accentColor),
			backgroundImage,
			frostedGlass: backgroundImage ? settings.frostedGlass : false
		});
	}

	function handleCancel() {
		settings = { ...$settingsStore };
	}

	function handleReset() {
		settingsStore.reset();
		settings = { ...$settingsStore };
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

	function handleBackgroundClear() {
		settings.backgroundImage = null;
		settings.frostedGlass = false;
	}

	function openWallpaperDialog() {
		wallpaperDialogOpen = true;
		wallpaperSource = null;
		serverWallpaperError = null;
		localWallpaperError = null;
	}

	function closeWallpaperDialog() {
		wallpaperDialogOpen = false;
	}

	async function chooseWallpaperSource(source: WallpaperSource) {
		wallpaperSource = source;
		serverWallpaperError = null;
		localWallpaperError = null;

		await loadServerWallpaperRoots();
	}

	async function loadServerWallpaperRoots() {
		serverWallpaperLoading = true;
		serverWallpaperError = null;

		try {
			const response = await listRoots();
			serverWallpaperRoots = response.roots;
			serverWallpaperPath = '';
			serverWallpaperItems = [];
		} catch (error) {
			serverWallpaperError =
				error instanceof Error ? error.message : 'Unable to load server folders.';
		} finally {
			serverWallpaperLoading = false;
		}
	}

	async function openServerWallpaperPath(path: string) {
		serverWallpaperPath = path;
		serverWallpaperLoading = true;
		serverWallpaperError = null;

		try {
			const response = await listDirectory(path, {
				page: 1,
				pageSize: 200,
				sortBy: 'name',
				sortDir: 'asc',
				includeHidden: settings.showHiddenFiles
			});
			serverWallpaperItems = response.items;
		} catch (error) {
			serverWallpaperError = error instanceof Error ? error.message : 'Unable to open this folder.';
			serverWallpaperItems = [];
		} finally {
			serverWallpaperLoading = false;
		}
	}

	function openServerWallpaperRoot() {
		serverWallpaperPath = '';
		serverWallpaperItems = [];
		serverWallpaperError = null;
	}

	async function openServerWallpaperParent() {
		if (!serverWallpaperPath) return;

		const parentPath = serverWallpaperPath.split('/').slice(0, -1).join('/');
		if (parentPath) {
			await openServerWallpaperPath(parentPath);
		} else {
			openServerWallpaperRoot();
		}
	}

	function isImageFile(item: FileInfo): boolean {
		return (
			!item.isDir && (item.mimeType?.startsWith('image/') || getPreviewType(item.name) === 'image')
		);
	}

	function chooseServerWallpaper(item: FileInfo) {
		const backgroundImage = toServerBackgroundImage(item.path);
		if (!backgroundImage) return;

		settings.backgroundImage = backgroundImage;
		closeWallpaperDialog();
	}

	function openLocalWallpaperPicker() {
		localWallpaperError = null;
		localWallpaperInput?.click();
	}

	async function handleLocalWallpaperChange(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';

		if (!file) return;

		localWallpaperError = null;
		if (!file.type.startsWith('image/') && getPreviewType(file.name) !== 'image') {
			localWallpaperError = 'Choose an image file.';
			return;
		}

		if (file.size > LOCAL_WALLPAPER_MAX_BYTES) {
			localWallpaperError = `Choose an image smaller than ${formatFileSize(LOCAL_WALLPAPER_MAX_BYTES)}.`;
			return;
		}

		try {
			settings.backgroundImage = await readFileAsDataUrl(file);
			closeWallpaperDialog();
		} catch {
			localWallpaperError = 'Unable to read this image.';
		}
	}

	function readFileAsDataUrl(file: File): Promise<string> {
		return new Promise((resolve, reject) => {
			const reader = new FileReader();
			reader.onload = () => {
				if (typeof reader.result === 'string') {
					resolve(reader.result);
				} else {
					reject(new Error('Unsupported file result'));
				}
			};
			reader.onerror = () => reject(reader.error ?? new Error('Unable to read file'));
			reader.readAsDataURL(file);
		});
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
					disabled={!hasChanges}
				>
					<X size={16} />
					<span class="hidden sm:inline">Cancel</span>
				</Button>
				<Button
					variant="primary"
					size="sm"
					onclick={handleSave}
					title="Save changes"
					disabled={!canSave}
				>
					<Save size={16} />
					<span class="hidden sm:inline">Save</span>
				</Button>
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

							{#if matchesSearch('background image', 'wallpaper', 'custom background', 'personalization')}
								<div class="{settingRowClass} flex-wrap">
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
										<Button variant="secondary" size="sm" onclick={openWallpaperDialog}>
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

							{#if matchesSearch('frosted glass', 'blur', 'background blur', 'glass look')}
								<div class={settingRowClass}>
									<div>
										<div class="flex items-center gap-2 text-[13px] text-text-primary">
											<Sparkles size={14} class="text-accent" />
											<span>Frosted glass blur</span>
										</div>
									</div>
									<Toggle
										bind:checked={settings.frostedGlass}
										disabled={!hasBackgroundImage}
										label="Frosted glass blur"
										showLabel={false}
									/>
								</div>
							{/if}
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

<input
	bind:this={localWallpaperInput}
	type="file"
	accept="image/*"
	class="hidden"
	onchange={handleLocalWallpaperChange}
/>

<Modal open={wallpaperDialogOpen} title="Choose Wallpaper" onclose={closeWallpaperDialog}>
	{#snippet headerActions()}
		{#if wallpaperSource === 'server'}
			<Button variant="ghost" size="sm" onclick={() => (wallpaperSource = null)}>Back</Button>
		{/if}
	{/snippet}

	{#if wallpaperSource === null}
		<div class="flex flex-col gap-4">
			<p class="m-0 text-sm text-text-secondary">Where should BoxBox pick the wallpaper from?</p>

			<div class="grid gap-3 sm:grid-cols-2">
				<button
					type="button"
					class="flex h-16 cursor-pointer items-center gap-3 rounded-lg border border-border-primary bg-surface-secondary px-4 py-3 text-left transition-colors hover:border-border-focus hover:bg-surface-tertiary"
					onclick={() => chooseWallpaperSource('server')}
				>
					<span class="rounded bg-accent/15 p-2 text-accent"><HardDrive size={22} /></span>
					<span>
						<span class="block text-sm font-medium text-text-primary">This Server</span>
					</span>
				</button>

				<button
					type="button"
					class="flex h-16 cursor-pointer items-center gap-3 rounded-lg border border-border-primary bg-surface-secondary px-4 py-3 text-left transition-colors hover:border-border-focus hover:bg-surface-tertiary"
					onclick={openLocalWallpaperPicker}
				>
					<span class="rounded bg-accent/15 p-2 text-accent"><Upload size={22} /></span>
					<span>
						<span class="block text-sm font-medium text-text-primary">This Device</span>
					</span>
				</button>
			</div>

			{#if localWallpaperError}
				<div
					class="flex items-start gap-2 rounded border border-danger/30 bg-danger/15 px-3 py-2 text-sm text-danger"
				>
					<AlertTriangle size={16} class="mt-0.5 shrink-0" />
					<span>{localWallpaperError}</span>
				</div>
			{/if}
		</div>
	{:else if wallpaperSource === 'server'}
		<div class="flex flex-col gap-3">
			{#if serverWallpaperPath}
				<div class="flex justify-end">
					<Button variant="ghost" size="sm" onclick={openServerWallpaperParent}>
						<ChevronUp size={14} />
						Up
					</Button>
				</div>
			{/if}

			<div
				class="rounded border border-border-primary bg-surface-secondary px-3 py-2 text-xs text-text-secondary"
			>
				<button
					type="button"
					class="cursor-pointer border-none bg-transparent p-0 text-text-primary hover:text-accent"
					onclick={openServerWallpaperRoot}
				>
					Server
				</button>
				{#each serverWallpaperCrumbs as crumb, index (`${crumb}-${index}`)}
					<span class="mx-1 text-text-muted">/</span>
					<span class={index === serverWallpaperCrumbs.length - 1 ? 'text-text-primary' : ''}
						>{crumb}</span
					>
				{/each}
			</div>

			{#if serverWallpaperError}
				<div
					class="flex items-start gap-2 rounded border border-danger/30 bg-danger/15 px-3 py-2 text-sm text-danger"
				>
					<AlertTriangle size={16} class="mt-0.5 shrink-0" />
					<span>{serverWallpaperError}</span>
				</div>
			{/if}

			{#if serverWallpaperLoading}
				<div
					class="rounded border border-border-primary bg-surface-secondary px-3 py-8 text-center text-sm text-text-secondary"
				>
					Loading server folders...
				</div>
			{:else if !serverWallpaperPath}
				<div
					class="max-h-72 overflow-auto rounded border border-border-primary bg-surface-secondary"
				>
					{#if serverWallpaperRoots.length === 0}
						<div class="px-3 py-8 text-center text-sm text-text-secondary">
							No server folders are available.
						</div>
					{:else}
						{#each serverWallpaperRoots as root (root.name)}
							<button
								type="button"
								class="flex w-full cursor-pointer items-center gap-3 border-0 border-b border-border-secondary bg-transparent px-3 py-2.5 text-left last:border-b-0 hover:bg-surface-tertiary"
								onclick={() => openServerWallpaperPath(root.name)}
							>
								<Folder size={16} class="shrink-0 text-accent" />
								<span class="min-w-0 flex-1 truncate text-sm text-text-primary">{root.name}</span>
								{#if root.readOnly}
									<span class="text-[11px] text-text-muted">Read only</span>
								{/if}
							</button>
						{/each}
					{/if}
				</div>
			{:else}
				<div
					class="max-h-72 overflow-auto rounded border border-border-primary bg-surface-secondary"
				>
					{#if serverWallpaperEntries.length === 0}
						<div class="px-3 py-8 text-center text-sm text-text-secondary">
							No image files found in this folder.
						</div>
					{:else}
						{#each serverWallpaperEntries as item (item.path)}
							{#if item.isDir}
								<button
									type="button"
									class="flex w-full cursor-pointer items-center gap-3 border-0 border-b border-border-secondary bg-transparent px-3 py-2.5 text-left last:border-b-0 hover:bg-surface-tertiary"
									onclick={() => openServerWallpaperPath(item.path)}
								>
									<Folder size={16} class="shrink-0 text-accent" />
									<span class="min-w-0 flex-1 truncate text-sm text-text-primary">{item.name}</span>
								</button>
							{:else}
								<button
									type="button"
									class="flex w-full cursor-pointer items-center gap-3 border-0 border-b border-border-secondary bg-transparent px-3 py-2.5 text-left last:border-b-0 hover:bg-surface-tertiary"
									onclick={() => chooseServerWallpaper(item)}
								>
									<FileImage size={16} class="shrink-0 text-accent" />
									<span class="min-w-0 flex-1 truncate text-sm text-text-primary">{item.name}</span>
									<span class="text-xs text-text-muted">{formatFileSize(item.size)}</span>
								</button>
							{/if}
						{/each}
					{/if}
				</div>
			{/if}
		</div>
	{/if}
</Modal>
