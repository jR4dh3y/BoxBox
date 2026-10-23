<script lang="ts">
	import { onDestroy } from 'svelte';
	import { SettingsOperations } from '#lib/utils/settingsOperation.js';
	import {
		AlertTriangle,
		Check,
		ChevronLeft,
		FileImage,
		Folder,
		HardDrive,
		Upload,
		Image as ImageIcon
	} from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { Button, Modal, ProgressBar, Select } from '#lib/components/ui/index.js';
	import { visibleByName } from '#lib/browse/sortFiles.js';
	import { listDirectory, listRoots, type FileInfo, type MountPoint } from '#lib/api/files.js';
	import { sortFiles } from '#lib/browse/sortFiles.js';
	import { resolveBackgroundImage, toServerBackgroundImage } from '#lib/utils/appearance.js';
	import { formatFileSize } from '#lib/utils/format.js';
	import {
		DEFAULT_BACKGROUND_IMAGE_MODE,
		WALLPAPER_DISPLAY_OPTIONS,
		getWallpaperBackgroundStyle,
		isWallpaperImageFile,
		normalizeBackgroundImageMode,
		readFileAsDataUrl,
		type BackgroundImageMode
	} from '#lib/utils/wallpaper.js';

	const operations = new SettingsOperations();
	onDestroy(() => operations.dispose());

	type WallpaperSource = 'server';
	type PreviewOrigin = 'server' | 'local';

	interface WallpaperSelection {
		backgroundImage: string;
		name: string;
		mode: BackgroundImageMode;
	}

	interface Props {
		open?: boolean;
		currentMode?: BackgroundImageMode;
		frostedGlass?: boolean;
		showHiddenFiles: boolean;
		onapply?: (selection: WallpaperSelection) => void;
		onclose?: () => void;
	}

	const LOCAL_WALLPAPER_MAX_BYTES = 20 * 1024 * 1024;
	const PREVIEW_ROWS = [0, 1, 2, 3, 4, 5];
	const SERVER_WALLPAPER_ITEMS_LIMIT = 200;

	let {
		open = true,
		currentMode = DEFAULT_BACKGROUND_IMAGE_MODE,
		frostedGlass = false,
		showHiddenFiles,
		onapply,
		onclose
	}: Props = $props();

	let wallpaperSource = $state<WallpaperSource | null>(null);
	let serverWallpaperRoots = $state<MountPoint[]>([]);
	let serverWallpaperPath = $state('');
	let serverWallpaperItems = $state<FileInfo[]>([]);
	let serverWallpaperLoad = $state.raw<AbortSignal | null>(null);
	let serverWallpaperError = $state<string | null>(null);
	let localWallpaperError = $state<string | null>(null);
	let localWallpaperUploading = $state(false);
	let localWallpaperProgress = $state(0);
	let localWallpaperProgressLabel = $state('');
	let previewBackgroundImage = $state<string | null>(null);
	let previewName = $state('');
	let previewOrigin = $state<PreviewOrigin | null>(null);
	let previewMode = $state<BackgroundImageMode>(DEFAULT_BACKGROUND_IMAGE_MODE);
	let localWallpaperInput: HTMLInputElement;

	const isPreviewing = $derived(previewBackgroundImage !== null);
	const title = $derived(isPreviewing ? 'Preview wallpaper' : 'Choose wallpaper');
	const modalSize = $derived(isPreviewing ? 'lg' : 'md');
	const serverWallpaperEntries = $derived.by(() =>
		serverWallpaperItems.filter((item) => item.isDir || isWallpaperImageFile(item))
	);
	const serverWallpaperCrumbs = $derived(
		serverWallpaperPath ? serverWallpaperPath.split('/').filter(Boolean) : []
	);
	const previewImageUrl = $derived(
		previewBackgroundImage ? resolveBackgroundImage(previewBackgroundImage) : null
	);
	const previewImageStyle = $derived(
		previewImageUrl ? `url(${JSON.stringify(previewImageUrl)})` : undefined
	);
	const previewBackgroundStyle = $derived(getWallpaperBackgroundStyle(previewMode));

	// The desktop app's folders and "this device" are the same disk, so it skips the choice.
	const desktop = isLocalDesktop();
	onMount(() => {
		if (desktop) void chooseWallpaperSource('server');
	});

	async function chooseWallpaperSource(source: WallpaperSource) {
		wallpaperSource = source;
		serverWallpaperError = null;
		localWallpaperError = null;

		if (serverWallpaperRoots.length === 0) {
			await loadServerWallpaperRoots();
		}
	}

	async function loadServerWallpaperRoots() {
		const signal = operations.start();
		serverWallpaperLoad = signal;
		serverWallpaperError = null;

		try {
			const response = await listRoots();
			signal.throwIfAborted();
			serverWallpaperRoots = response.roots;
		} catch (error) {
			if (signal.aborted) return;
			serverWallpaperError =
				error instanceof Error ? error.message : 'Unable to load server folders.';
		} finally {
			if (serverWallpaperLoad === signal) serverWallpaperLoad = null;
		}
	}

	async function openServerWallpaperPath(path: string) {
		const signal = operations.start();
		serverWallpaperPath = path;
		serverWallpaperItems = [];
		serverWallpaperLoad = signal;
		serverWallpaperError = null;

		try {
			const response = await listDirectory(path, signal);
			signal.throwIfAborted();
			serverWallpaperItems = visibleByName(response.items, showHiddenFiles).slice(
				0,
				SERVER_WALLPAPER_ITEMS_LIMIT
			);
		} catch (error) {
			if (signal.aborted) return;
			serverWallpaperError = error instanceof Error ? error.message : 'Unable to open this folder.';
			serverWallpaperItems = [];
		} finally {
			if (serverWallpaperLoad === signal) serverWallpaperLoad = null;
		}
	}

	function openServerWallpaperRoot() {
		operations.start();
		serverWallpaperLoad = null;
		if (serverWallpaperRoots.length === 0) void loadServerWallpaperRoots();
		serverWallpaperPath = '';
		serverWallpaperItems = [];
		serverWallpaperError = null;
	}

	function previewServerWallpaper(item: FileInfo) {
		if (operations.start().aborted) return;
		const backgroundImage = toServerBackgroundImage(item.path);
		if (!backgroundImage) return;

		previewBackgroundImage = backgroundImage;
		previewName = item.name;
		previewOrigin = 'server';
		previewMode = normalizeBackgroundImageMode(currentMode);
	}

	function openLocalWallpaperPicker() {
		if (localWallpaperUploading) return;

		localWallpaperError = null;
		localWallpaperInput?.click();
	}

	async function handleLocalWallpaperChange(event: Event & { currentTarget: HTMLInputElement }) {
		const signal = operations.start();
		const input = event.currentTarget;
		const file = input.files?.[0];
		input.value = '';

		if (!file || localWallpaperUploading) return;

		localWallpaperError = null;
		localWallpaperProgress = 0;
		localWallpaperProgressLabel = '';
		if (!file.type.startsWith('image/') && !isLocalImageFile(file.name)) {
			localWallpaperError = 'Choose an image file.';
			return;
		}

		if (file.size > LOCAL_WALLPAPER_MAX_BYTES) {
			localWallpaperError = `Choose an image smaller than ${formatFileSize(LOCAL_WALLPAPER_MAX_BYTES)} so it can be saved locally.`;
			return;
		}

		try {
			localWallpaperUploading = true;
			localWallpaperProgressLabel = 'Preparing wallpaper upload...';
			const image = await readFileAsDataUrl(file, (progress) => {
				if (signal.aborted) return;
				localWallpaperProgress = progress;
				localWallpaperProgressLabel =
					progress >= 100 ? 'Preparing preview...' : 'Uploading wallpaper...';
			});
			signal.throwIfAborted();
			previewBackgroundImage = image;
			previewName = file.name;
			previewOrigin = 'local';
			previewMode = normalizeBackgroundImageMode(currentMode);
		} catch {
			if (signal.aborted) return;
			localWallpaperError = 'Unable to read this image.';
			localWallpaperProgress = 0;
			localWallpaperProgressLabel = '';
		} finally {
			localWallpaperUploading = false;
		}
	}

	function isLocalImageFile(name: string): boolean {
		const extension = name.split('.').pop()?.toLowerCase();
		return ['avif', 'gif', 'jpeg', 'jpg', 'png', 'svg', 'webp'].includes(extension ?? '');
	}

	function returnFromPreview() {
		operations.start();
		const origin = previewOrigin;
		previewBackgroundImage = null;
		previewName = '';
		previewOrigin = null;
		localWallpaperProgress = 0;
		localWallpaperProgressLabel = '';

		if (origin === 'server' || desktop) {
			wallpaperSource = 'server';
		} else {
			wallpaperSource = null;
		}
	}

	function applyPreview() {
		if (operations.start().aborted || !previewBackgroundImage) return;

		onapply?.({
			backgroundImage: previewBackgroundImage,
			name: previewName,
			mode: normalizeBackgroundImageMode(previewMode)
		});
	}
</script>

<input
	bind:this={localWallpaperInput}
	type="file"
	accept="image/*"
	class="hidden"
	onchange={handleLocalWallpaperChange}
/>

<Modal {open} {title} size={modalSize} {onclose}>
	{#snippet headerActions()}
		{#if isPreviewing}
			<Button variant="ghost" size="sm" onclick={returnFromPreview}>
				<ChevronLeft size={14} />
				{previewOrigin === 'server' ? 'Back to folder' : 'Back'}
			</Button>
		{:else if wallpaperSource === 'server' && !desktop}
			<Button variant="ghost" size="sm" onclick={() => (wallpaperSource = null)}>
				<ChevronLeft size={14} />
				Sources
			</Button>
		{/if}
	{/snippet}

	{#if isPreviewing}
		<div class="flex flex-col gap-4">
			<div
				class="relative h-85 overflow-hidden rounded-lg border border-border-primary bg-surface-secondary"
				style:background-image={previewImageStyle}
				style:background-size={previewBackgroundStyle.size}
				style:background-repeat={previewBackgroundStyle.repeat}
				style:background-position={previewBackgroundStyle.position}
			>
				<div class="wallpaper-scrim absolute inset-0"></div>
				<div
					class="wallpaper-preview relative flex h-full overflow-hidden rounded"
					data-frosted-glass={frostedGlass ? 'true' : undefined}
				>
					<div class="browse-chrome w-28 shrink-0 border-r border-white/10 p-3 sm:w-36">
						<div class="space-y-2">
							<div class="preview-field h-7 rounded"></div>
							<div class="preview-field preview-field-accent h-7 rounded"></div>
							<div class="preview-field h-7 rounded"></div>
							<div class="preview-field h-7 rounded"></div>
						</div>
					</div>
					<div class="flex min-w-0 flex-1 flex-col">
						<div class="browse-chrome flex h-12 items-center gap-2 border-b border-white/10 px-3">
							<div class="preview-field h-7 w-7 rounded"></div>
							<div class="preview-field h-7 flex-1 rounded"></div>
							<div class="preview-field h-7 w-20 rounded"></div>
						</div>
						<div class="flex-1 p-4">
							<div class="overflow-hidden rounded border border-white/10">
								{#each PREVIEW_ROWS as row (`preview-row-${row}`)}
									<div
										class="grid grid-cols-[minmax(0,1fr)_minmax(30px,90px)_minmax(30px,110px)] items-center gap-4 border-b border-white/10 px-3 py-3 last:border-b-0"
									>
										<div class="flex items-center gap-2">
											<div class="preview-field preview-field-accent h-4 w-4 rounded"></div>
											<div class="preview-field preview-field-strong h-2.5 w-32 rounded"></div>
										</div>
										<div class="preview-field preview-field-muted h-2.5 rounded"></div>
										<div class="preview-field preview-field-muted h-2.5 rounded"></div>
									</div>
								{/each}
							</div>
						</div>
					</div>
				</div>
			</div>

			<div class="flex flex-wrap items-center justify-end gap-3">
				<div
					class="group relative flex min-w-0 flex-1 items-center gap-2 text-sm font-medium text-text-primary"
				>
					<ImageIcon size={16} class="shrink-0 text-accent" />
					<span class="block w-56 truncate" title={previewName}>{previewName}</span>
					<span
						class="pointer-events-none absolute top-6 left-6 z-10 hidden max-w-md rounded border border-border-primary bg-surface-elevated px-2 py-1 text-xs font-normal whitespace-normal text-text-primary shadow-lg group-hover:block"
					>
						{previewName}
					</span>
				</div>
				<div class="w-48 shrink-0">
					<Select
						id="wallpaper-preview-mode"
						options={WALLPAPER_DISPLAY_OPTIONS}
						bind:value={previewMode}
					/>
				</div>
				<Button onclick={applyPreview}>
					<Check size={16} />
					Use wallpaper
				</Button>
			</div>
		</div>
	{:else if wallpaperSource === null}
		<div class="flex flex-col gap-4">
			<div class="flex flex-col gap-3">
				<button
					type="button"
					class="flex cursor-pointer items-center gap-3 rounded-lg border border-border-primary bg-surface-secondary p-3 text-left transition-colors hover:bg-surface-tertiary"
					onclick={() => chooseWallpaperSource('server')}
				>
					<span
						class="flex h-10 w-10 shrink-0 items-center justify-center rounded bg-accent-base/10 text-accent-base"
					>
						<HardDrive size={22} />
					</span>
					<span class="flex flex-col gap-0.5">
						<span class="text-[13px] text-text-primary">This Server</span>
						<span class="text-xs text-text-muted">Browse mounted folders</span>
					</span>
				</button>
				<button
					type="button"
					class="flex cursor-pointer items-center gap-3 rounded-lg border border-border-primary bg-surface-secondary p-3 text-left transition-colors hover:bg-surface-tertiary disabled:cursor-not-allowed disabled:opacity-60"
					onclick={openLocalWallpaperPicker}
					disabled={localWallpaperUploading}
					aria-busy={localWallpaperUploading}
				>
					<span
						class="flex h-10 w-10 shrink-0 items-center justify-center rounded bg-accent-base/10 text-accent-base"
					>
						<Upload size={22} />
					</span>
					<span class="flex flex-col gap-0.5">
						<span class="text-[13px] text-text-primary">This Device</span>
						<span class="text-xs text-text-muted">Upload a local image</span>
					</span>
				</button>
			</div>

			{#if localWallpaperUploading}
				<div
					class="rounded border border-border-primary bg-surface-secondary px-3 py-2"
					role="status"
					aria-live="polite"
				>
					<div class="mb-2 flex items-center justify-between gap-3 text-xs">
						<span class="text-text-secondary">{localWallpaperProgressLabel}</span>
						<span class="shrink-0 font-medium text-accent">{localWallpaperProgress}%</span>
					</div>
					<ProgressBar value={localWallpaperProgress} size="sm" />
				</div>
			{/if}

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
			<div
				class="min-w-0 rounded border border-border-primary bg-surface-secondary px-3 py-2 text-xs text-text-secondary"
			>
				<button
					type="button"
					class="cursor-pointer border-none bg-transparent p-0 text-text-primary hover:text-accent"
					onclick={openServerWallpaperRoot}
				>
					{rootLabel()}
				</button>
				{#each serverWallpaperCrumbs as crumb, index (`${crumb}-${index}`)}
					<span class="mx-1 text-text-muted">/</span>
					<button
						type="button"
						class="cursor-pointer border-none bg-transparent p-0 {index ===
						serverWallpaperCrumbs.length - 1
							? 'text-text-primary'
							: 'text-text-secondary'} hover:text-accent"
						onclick={() =>
							openServerWallpaperPath(serverWallpaperCrumbs.slice(0, index + 1).join('/'))}
					>
						{crumb}
					</button>
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

			{#if serverWallpaperLoad}
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
									onclick={() => previewServerWallpaper(item)}
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

<style>
	.preview-field {
		background: rgb(255 255 255 / 0.12);
	}

	.preview-field-muted {
		background: rgb(255 255 255 / 0.2);
	}

	.preview-field-strong {
		background: rgb(255 255 255 / 0.35);
	}

	.preview-field-accent {
		background: color-mix(in srgb, var(--color-accent) 55%, transparent);
	}
</style>
