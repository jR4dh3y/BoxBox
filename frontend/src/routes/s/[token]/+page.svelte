<script lang="ts">
	/** Public recipient page for file and folder share links: path bar on top, file list and preview below. */
	import { onDestroy, onMount } from 'svelte';
	import { page } from '$app/state';
	import { Archive, Download, Trash2, Upload } from 'lucide-svelte';
	import { Button, ProgressBar, Spinner, Toast } from '$lib/components/ui';
	import {
		ApiRequestError,
		getShareInfo,
		hasShareExpiry,
		deleteShareItem,
		listShareItems,
		shareArchiveUrl,
		shareDownloadUrl,
		sharePreviewUrl,
		shareUploadUrl,
		shareWallpaperUrl,
		type ShareInfoResponse,
		type ShareItem
	} from '$lib/api';
	import { shareWallpaperStore } from '$lib/stores/shareWallpaper.svelte';
	import { toastStore } from '$lib/stores/toast.svelte';
	import { getFileIcon, getPreviewType } from '$lib/utils/fileTypes';
	import { formatFileSize, formatRelativeTime } from '$lib/utils/format';
	import { getShareAccessLabel } from '$lib/utils/shareAccess';

	interface SelectedFile {
		/** Path inside the shared folder; undefined for a single-file share. */
		path?: string;
		name: string;
		size: number;
	}

	const TEXT_PREVIEW_LIMIT_BYTES = 1024 * 1024;
	const token = $derived(page.params.token ?? '');

	let info = $state<ShareInfoResponse | null>(null);
	let loading = $state(true);
	let gone = $state(false);
	let loadError = $state<string | null>(null);
	let folderPath = $state('');
	let folderItems = $state<ShareItem[]>([]);
	let folderLoading = $state(false);
	let folderError = $state<string | null>(null);
	let folderRequestId = 0;
	let selected = $state<SelectedFile | null>(null);
	let textContent = $state<string | null>(null);
	let textFailed = $state(false);
	let textRequestId = 0;
	let uploadInput: HTMLInputElement;
	let uploadingName = $state<string | null>(null);
	let uploadProgress = $state(0);

	const breadcrumbs = $derived(folderPath ? folderPath.split('/') : []);
	const previewType = $derived(selected ? getPreviewType(selected.name) : 'unsupported');
	const isTextPreview = $derived(previewType === 'code' || previewType === 'text');
	const previewUrl = $derived(selected ? sharePreviewUrl(token, selected.path) : '');
	const linkSummary = $derived.by(() => {
		if (!info) return '';
		const parts = info.isFolder
			? [getShareAccessLabel(info.permissions)]
			: [formatFileSize(info.size), 'View only'];
		if (info.permissions.upload) parts.push(`${formatFileSize(info.maxUploadBytes)} per file`);
		parts.push(
			hasShareExpiry(info.expiresAt)
				? `Expires ${formatRelativeTime(info.expiresAt)}`
				: 'Never expires'
		);
		return parts.join(' · ');
	});

	const panelClass =
		'flex min-h-0 flex-col overflow-hidden rounded-lg border border-border-primary bg-surface-secondary';
	const crumbClass =
		'min-w-0 shrink cursor-pointer truncate border-none bg-transparent p-0 text-[13px] text-text-secondary hover:text-text-primary';
	const iconButtonClass =
		'flex h-7 w-7 shrink-0 cursor-pointer items-center justify-center rounded border-none bg-transparent text-text-secondary transition-colors duration-100 hover:bg-surface-elevated hover:text-text-primary disabled:cursor-not-allowed disabled:opacity-50';

	onMount(() => {
		void loadShare();
	});

	onDestroy(() => (shareWallpaperStore.current = null));

	async function loadShare() {
		loading = true;
		gone = false;
		loadError = null;
		info = null;
		selected = null;
		folderRequestId += 1;
		folderPath = '';
		folderItems = [];
		try {
			const shareInfo = await getShareInfo(token);
			info = shareInfo;
			shareWallpaperStore.current = shareInfo.wallpaper
				? {
						url: shareWallpaperUrl(token, shareInfo.wallpaper.version),
						mode: shareInfo.wallpaper.mode,
						frostedGlass: shareInfo.wallpaper.frostedGlass
					}
				: null;
			if (shareInfo.isFolder) {
				await loadFolder('');
			} else {
				selected = { name: shareInfo.fileName, size: shareInfo.size };
			}
		} catch (error) {
			if (error instanceof ApiRequestError && error.status === 404) {
				gone = true;
			} else {
				loadError = error instanceof Error ? error.message : 'Unable to load this share.';
			}
		} finally {
			loading = false;
		}
	}

	async function loadFolder(path: string) {
		const requestId = ++folderRequestId;
		folderLoading = true;
		folderError = null;
		try {
			const response = await listShareItems(token, path);
			if (requestId !== folderRequestId) return;
			folderPath = response.path;
			folderItems = response.items;
		} catch (error) {
			if (requestId !== folderRequestId) return;
			if (error instanceof ApiRequestError && error.status === 404) {
				gone = true;
			} else {
				folderError = error instanceof Error ? error.message : 'Unable to load this folder.';
			}
		} finally {
			if (requestId === folderRequestId) folderLoading = false;
		}
	}

	$effect(() => {
		const url = previewUrl;
		textContent = null;
		textFailed = false;
		if (url && isTextPreview && info?.permissions.view) void loadTextPreview(url);
	});

	async function loadTextPreview(url: string) {
		const requestId = ++textRequestId;
		try {
			const response = await fetch(url);
			if (!response.ok) throw new Error('preview request failed');
			const text = (await response.text()).slice(0, TEXT_PREVIEW_LIMIT_BYTES);
			if (requestId === textRequestId) textContent = text;
		} catch {
			if (requestId === textRequestId) textFailed = true;
		}
	}

	function openItem(item: ShareItem) {
		if (item.isDir) {
			selected = null;
			void loadFolder(item.path);
		} else {
			selected = { path: item.path, name: item.name, size: item.size };
		}
	}

	function goToBreadcrumb(index: number) {
		selected = null;
		void loadFolder(index < 0 ? '' : breadcrumbs.slice(0, index + 1).join('/'));
	}

	async function handleDeleteItem(item: ShareItem) {
		if (!window.confirm(`Delete ${item.name} from this shared folder?`)) return;
		try {
			await deleteShareItem(token, item.path);
			if (selected?.path === item.path) selected = null;
			await loadFolder(folderPath);
		} catch (error) {
			if (error instanceof ApiRequestError && error.status === 404) {
				gone = true;
				return;
			}
			toastStore.error(error instanceof Error ? error.message : 'Unable to delete this item');
		}
	}

	function handleUploadChange(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (file && !uploadingName) uploadFile(file);
	}

	function uploadFile(file: File) {
		if (!info?.permissions.upload) return;
		if (info.maxUploadBytes > 0 && file.size > info.maxUploadBytes) {
			toastStore.error(`This link allows files up to ${formatFileSize(info.maxUploadBytes)}.`);
			return;
		}
		const relativePath = folderPath ? `${folderPath}/${file.name}` : file.name;
		uploadingName = file.name;
		uploadProgress = 0;
		const xhr = new XMLHttpRequest();
		xhr.open('POST', shareUploadUrl(token, relativePath));
		xhr.upload.onprogress = (event) => {
			if (event.lengthComputable) {
				uploadProgress = Math.round((event.loaded / event.total) * 100);
			}
		};
		xhr.onload = () => {
			uploadingName = null;
			if (xhr.status >= 200 && xhr.status < 300) {
				void loadFolder(folderPath);
			} else if (xhr.status === 403) {
				toastStore.error('This link cannot replace an existing file.');
			} else if (xhr.status === 413) {
				toastStore.error('File too large');
			} else if (xhr.status === 404) {
				gone = true;
			} else {
				toastStore.error('Unable to add this file');
			}
		};
		xhr.onerror = () => {
			uploadingName = null;
			toastStore.error('Unable to add this file');
		};
		xhr.send(file);
	}
</script>

<svelte:head>
	<title>{info?.fileName ?? 'Shared'} - BoxBox</title>
</svelte:head>

<input bind:this={uploadInput} type="file" class="hidden" onchange={handleUploadChange} />

<!-- Without an owner wallpaper the page is plain; with one, it shows between the panels. -->
<div
	class="flex h-screen flex-col gap-3 p-3 text-text-primary {shareWallpaperStore.current
		? ''
		: 'bg-surface-primary'}"
>
	<header
		class="flex shrink-0 items-center gap-2 rounded-lg border border-border-primary bg-surface-primary px-3 py-1.5"
	>
		<img src="/logo-boxbox-3d.svg" alt="BoxBox" class="mr-1 h-8 w-auto shrink-0" />
		<nav
			class="flex min-w-0 flex-1 items-center gap-1.5 rounded border border-border-primary bg-surface-secondary px-2 py-1 text-[13px] whitespace-nowrap"
			aria-label="Path"
		>
			{#if info}
				{#if info.isFolder}
					{#if breadcrumbs.length || selected}
						<button type="button" class={crumbClass} onclick={() => goToBreadcrumb(-1)}
							>{info.fileName}</button
						>
					{:else}
						<span class="truncate text-text-primary">{info.fileName}</span>
					{/if}
					{#each breadcrumbs as crumb, index (index)}
						<span class="text-xs text-text-muted">/</span>
						{#if index < breadcrumbs.length - 1 || selected}
							<button type="button" class={crumbClass} onclick={() => goToBreadcrumb(index)}
								>{crumb}</button
							>
						{:else}
							<span class="truncate text-text-primary">{crumb}</span>
						{/if}
					{/each}
				{/if}
				{#if selected}
					{#if info.isFolder}<span class="text-xs text-text-muted">/</span>{/if}
					<span class="truncate text-text-primary" title={selected.name}>{selected.name}</span>
				{/if}
			{/if}
		</nav>
		{#if info?.permissions.download && selected}
			<a
				class={iconButtonClass}
				href={shareDownloadUrl(token, selected.path)}
				title="Download {selected.name}"
				aria-label="Download {selected.name}"><Download size={16} /></a
			>
		{/if}
		{#if info?.isFolder && info.permissions.download}
			<a
				class={iconButtonClass}
				href={shareArchiveUrl(token, folderPath || undefined)}
				title="Download folder as ZIP"
				aria-label="Download folder as ZIP"><Archive size={16} /></a
			>
		{/if}
		{#if info?.permissions.upload}
			<button
				type="button"
				class={iconButtonClass}
				title="Upload a file"
				aria-label="Upload a file"
				disabled={uploadingName !== null}
				onclick={() => uploadInput?.click()}><Upload size={16} /></button
			>
		{/if}
	</header>

	{#if loading || gone || loadError}
		<div class="{panelClass} flex-1 items-center justify-center gap-3 px-6 text-center">
			{#if loading}
				<Spinner />
			{:else if gone}
				<p class="m-0 text-sm text-text-primary">This share is no longer available.</p>
			{:else}
				<p class="m-0 text-sm text-text-secondary">{loadError}</p>
				<Button variant="secondary" size="sm" onclick={() => void loadShare()}>Retry</Button>
			{/if}
		</div>
	{:else if info}
		<div class="flex min-h-0 flex-1 flex-col gap-3 md:flex-row">
			{#if info.isFolder}
				<aside class="{panelClass} max-h-[45vh] md:max-h-none md:w-80 md:shrink-0">
					<div class="min-h-0 flex-1 overflow-auto py-1">
						{#if folderError}
							<div class="flex flex-col items-center gap-2 px-3 py-8 text-center">
								<p class="m-0 text-[13px] text-danger">{folderError}</p>
								<Button variant="secondary" size="sm" onclick={() => void loadFolder(folderPath)}
									>Retry</Button
								>
							</div>
						{:else if folderLoading && folderItems.length === 0}
							<div class="flex justify-center py-8"><Spinner /></div>
						{:else if folderItems.length === 0}
							<p class="m-0 px-3 py-8 text-center text-[13px] text-text-muted">Empty folder</p>
						{:else}
							{#each folderItems as item (item.path)}
								{@const ItemIcon = getFileIcon(item.name, item.isDir)}
								{@const isSelected = !item.isDir && selected?.path === item.path}
								<div
									class="group flex h-8 items-center text-[13px] {isSelected
										? 'bg-selection text-white'
										: 'hover:bg-surface-tertiary'}"
								>
									<button
										type="button"
										class="flex h-full min-w-0 flex-1 cursor-pointer items-center gap-2 border-none bg-transparent px-3 text-left text-inherit"
										onclick={() => openItem(item)}
									>
										<span
											class="flex w-5 shrink-0 justify-center {item.isDir
												? 'text-folder'
												: isSelected
													? 'text-white'
													: 'text-text-secondary'}"><ItemIcon size={16} /></span
										>
										<span class="min-w-0 flex-1 truncate {item.isDir ? 'text-folder' : ''}"
											>{item.name}</span
										>
										{#if !item.isDir}
											<span
												class="shrink-0 text-xs tabular-nums {isSelected
													? 'text-white/70'
													: 'text-text-muted'}">{formatFileSize(item.size)}</span
											>
										{/if}
									</button>
									{#if info.permissions.delete}
										<button
											type="button"
											class="mr-1 hidden h-7 w-7 shrink-0 cursor-pointer items-center justify-center rounded border-none bg-transparent text-text-secondary group-hover:flex hover:bg-danger/15 hover:text-danger"
											title="Delete {item.name}"
											aria-label="Delete {item.name}"
											onclick={() => void handleDeleteItem(item)}><Trash2 size={15} /></button
										>
									{/if}
								</div>
							{/each}
						{/if}
					</div>

					<div
						class="shrink-0 border-t border-border-secondary px-3 py-1.5 text-xs text-text-secondary"
					>
						{#if uploadingName}
							<div class="flex items-center gap-2">
								<span class="min-w-0 flex-1 truncate">Uploading {uploadingName}</span>
								<span class="tabular-nums">{uploadProgress}%</span>
							</div>
							<div class="mt-1"><ProgressBar value={uploadProgress} size="sm" /></div>
						{:else}
							{linkSummary}
						{/if}
					</div>
				</aside>
			{/if}

			<main class="{panelClass} flex-1">
				{#if selected}
					<div class="flex min-h-0 flex-1 items-center justify-center overflow-auto">
						{#if !info.permissions.view}
							<p class="m-0 text-[13px] text-text-muted">Preview is turned off for this link.</p>
						{:else if previewType === 'image'}
							<img
								src={previewUrl}
								alt={selected.name}
								class="max-h-full max-w-full object-contain"
							/>
						{:else if previewType === 'video'}
							<!-- svelte-ignore a11y_media_has_caption -->
							<video
								src={previewUrl}
								controls
								preload="metadata"
								playsinline
								class="max-h-full max-w-full"
							></video>
						{:else if previewType === 'audio'}
							<audio src={previewUrl} controls preload="metadata" class="w-full max-w-md"></audio>
						{:else if previewType === 'pdf'}
							<iframe
								sandbox=""
								src={previewUrl}
								title={selected.name}
								class="h-full w-full border-0"
							></iframe>
						{:else if isTextPreview}
							{#if textContent !== null}
								<pre
									class="m-0 h-full w-full overflow-auto p-4 text-xs leading-5 break-words whitespace-pre-wrap text-text-primary">{textContent}</pre>
							{:else if textFailed}
								<p class="m-0 text-[13px] text-text-muted">Preview unavailable.</p>
							{:else}
								<Spinner />
							{/if}
						{:else}
							<p class="m-0 text-[13px] text-text-muted">No preview for this file type.</p>
						{/if}
					</div>

					{#if !info.isFolder}
						<div
							class="shrink-0 border-t border-border-secondary px-3 py-1.5 text-xs text-text-secondary"
						>
							{linkSummary}
						</div>
					{/if}
				{:else}
					<div class="flex flex-1 items-center justify-center">
						<p class="m-0 text-[13px] text-text-muted">Select a file to preview</p>
					</div>
				{/if}
			</main>
		</div>
	{/if}
</div>

<Toast />
