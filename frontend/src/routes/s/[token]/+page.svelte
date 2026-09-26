<script lang="ts">
	/** Public recipient page for file and folder share links. */
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import {
		AlertTriangle,
		Archive,
		ChevronRight,
		Download,
		FileText,
		FolderOpen,
		Home,
		Upload,
		Trash2
	} from 'lucide-svelte';
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
		type ShareInfoResponse,
		type ShareItem
	} from '$lib/api';
	import { toastStore } from '$lib/stores/toast.svelte';
	import { getFileIcon, getPreviewType, getFileTypeDescription } from '$lib/utils/fileTypes';
	import { formatFileSize, formatRelativeTime } from '$lib/utils/format';
	import { getShareAccessLabel } from '$lib/utils/shareAccess';

	const TEXT_PREVIEW_LIMIT_BYTES = 1024 * 1024;
	const token = $derived(page.params.token ?? '');

	let info = $state<ShareInfoResponse | null>(null);
	let loading = $state(true);
	let gone = $state(false);
	let loadError = $state<string | null>(null);
	let textContent = $state<string | null>(null);
	let textFailed = $state(false);
	let uploadInput: HTMLInputElement;
	let uploading = $state(false);
	let uploadProgress = $state(0);
	let folderPath = $state('');
	let folderItems = $state<ShareItem[]>([]);
	let folderLoading = $state(false);
	let folderError = $state<string | null>(null);
	let folderRequestId = 0;

	const previewType = $derived(info ? getPreviewType(info.fileName) : 'unsupported');
	const previewUrl = $derived(token ? sharePreviewUrl(token) : '');
	const downloadUrl = $derived(token ? shareDownloadUrl(token) : '');
	const isTextPreview = $derived(previewType === 'code' || previewType === 'text');
	const TypeIcon = $derived(info ? getFileIcon(info.fileName, false) : FolderOpen);
	const folderBreadcrumbs = $derived(folderPath ? folderPath.split('/') : []);

	onMount(() => {
		void loadShare();
	});

	async function loadShare() {
		loading = true;
		gone = false;
		loadError = null;
		info = null;
		textContent = null;
		textFailed = false;
		folderRequestId += 1;
		folderPath = '';
		folderItems = [];
		try {
			const shareInfo = await getShareInfo(token);
			info = shareInfo;
			if (shareInfo.isFolder) await loadFolder('');
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
				folderError = null;
			} else {
				folderError = error instanceof Error ? error.message : 'Unable to load this folder.';
			}
		} finally {
			if (requestId === folderRequestId) folderLoading = false;
		}
	}

	$effect(() => {
		if (info && !info.isFolder && isTextPreview && textContent === null && !textFailed) {
			void loadTextPreview();
		}
	});

	async function loadTextPreview() {
		try {
			const response = await fetch(sharePreviewUrl(token));
			if (!response.ok) throw new Error('preview request failed');
			textContent = (await response.text()).slice(0, TEXT_PREVIEW_LIMIT_BYTES);
		} catch {
			textFailed = true;
		}
	}

	function handleDownload(itemPath?: string) {
		window.open(itemPath ? shareDownloadUrl(token, itemPath) : downloadUrl, '_blank');
	}

	function handleArchiveDownload() {
		window.open(shareArchiveUrl(token, folderPath || undefined), '_blank');
	}

	async function handleDeleteItem(item: ShareItem) {
		if (
			!info?.permissions.delete ||
			!window.confirm(`Delete ${item.name} from this shared folder?`)
		) {
			return;
		}
		try {
			await deleteShareItem(token, item.path);
			toastStore.success(`${item.name} deleted from the shared folder`);
			await loadFolder(folderPath);
		} catch (error) {
			if (error instanceof ApiRequestError && error.status === 404) {
				gone = true;
				return;
			}
			toastStore.error(error instanceof Error ? error.message : 'Unable to delete this item');
		}
	}

	function openFolderItem(item: ShareItem) {
		if (item.isDir) {
			void loadFolder(item.path);
			return;
		}
		window.open(sharePreviewUrl(token, item.path), '_blank');
	}

	function goToBreadcrumb(index: number) {
		void loadFolder(folderBreadcrumbs.slice(0, index + 1).join('/'));
	}

	function goToFolderRoot() {
		void loadFolder('');
	}

	function handleUploadClick() {
		if (!uploading) uploadInput?.click();
	}

	function handleUploadChange(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0];
		input.value = '';
		if (!file || uploading) return;
		uploadFile(file);
	}

	function uploadFile(file: File) {
		if (!info?.permissions.upload) return;
		if (info.maxUploadBytes > 0 && file.size > info.maxUploadBytes) {
			toastStore.error(`This link allows files up to ${formatFileSize(info.maxUploadBytes)}.`);
			return;
		}
		const relativePath = folderPath ? `${folderPath}/${file.name}` : file.name;
		uploading = true;
		uploadProgress = 0;
		const xhr = new XMLHttpRequest();
		xhr.open('POST', shareUploadUrl(token, relativePath));
		xhr.upload.onprogress = (event) => {
			if (event.lengthComputable) {
				uploadProgress = Math.round((event.loaded / event.total) * 100);
			}
		};
		xhr.onload = () => {
			uploading = false;
			if (xhr.status >= 200 && xhr.status < 300) {
				toastStore.success('File added to the shared folder');
				void loadFolder(folderPath);
			} else if (xhr.status === 403) {
				toastStore.error(
					info?.permissions.upload
						? 'This link cannot replace an existing file.'
						: 'This shared folder is view-only'
				);
			} else if (xhr.status === 413) {
				toastStore.error('File too large');
			} else if (xhr.status === 404) {
				gone = true;
			} else {
				toastStore.error('Unable to add this file');
			}
		};
		xhr.onerror = () => {
			uploading = false;
			toastStore.error('Unable to add this file');
		};
		xhr.send(file);
	}
</script>

<svelte:head>
	<title>{info?.isFolder ? 'Shared Folder' : 'Shared File'} - BoxBox</title>
</svelte:head>

<input bind:this={uploadInput} type="file" class="hidden" onchange={handleUploadChange} />

<div class="min-h-screen bg-surface-primary">
	<header class="mx-auto flex w-full max-w-[1180px] items-center px-4 py-6 sm:px-8">
		<div class="flex items-center gap-2 text-text-primary">
			<span class="text-accent"><FolderOpen size={24} /></span>
			<span class="text-base font-semibold tracking-tight">BoxBox</span>
		</div>
	</header>

	<main class="mx-auto w-full max-w-[1180px] px-4 pb-10 sm:px-8">
		<div class="rounded-lg border border-border-primary bg-surface-secondary shadow">
			{#if loading}
				<div class="flex flex-col items-center gap-4 py-24">
					<Spinner size="lg" />
					<span class="text-sm text-text-secondary">Loading share...</span>
				</div>
			{:else if gone}
				<div class="flex flex-col items-center gap-3 px-6 py-24 text-center">
					<AlertTriangle size={40} class="text-warning" />
					<h1 class="m-0 text-lg font-semibold text-text-primary">
						This share is no longer available
					</h1>
					<p class="m-0 text-sm text-text-secondary">
						The link may have expired or been revoked by its owner.
					</p>
				</div>
			{:else if loadError}
				<div class="flex flex-col items-center gap-4 px-6 py-24 text-center">
					<AlertTriangle size={40} class="text-danger" />
					<p class="m-0 text-sm text-text-secondary">{loadError}</p>
					<Button variant="secondary" size="sm" onclick={() => void loadShare()}>Retry</Button>
				</div>
			{:else if info?.isFolder}
				<section class="p-5 sm:p-8">
					<div class="flex flex-wrap items-start justify-between gap-4">
						<div class="flex min-w-0 items-center gap-3">
							<span class="shrink-0 text-folder"><FolderOpen size={34} /></span>
							<div class="min-w-0">
								<h1
									class="m-0 truncate text-xl font-semibold text-text-primary"
									title={info.fileName}
								>
									{info.fileName}
								</h1>
								<p class="m-0 text-sm text-text-secondary">
									Shared folder · {getShareAccessLabel(info.permissions)} access
								</p>
							</div>
						</div>
						<p class="m-0 shrink-0 text-right text-xs text-text-muted">
							{#if hasShareExpiry(info.expiresAt)}Expires {formatRelativeTime(
									info.expiresAt
								)}{:else}Never expires{/if}
						</p>
					</div>

					<div
						class="mt-7 flex flex-wrap items-center justify-between gap-3 border-b border-border-secondary pb-3"
					>
						<nav class="flex min-w-0 items-center gap-1 text-sm" aria-label="Shared folder path">
							<button
								type="button"
								class="rounded p-1 text-text-secondary hover:bg-surface-tertiary hover:text-text-primary"
								aria-label="Shared folder root"
								onclick={goToFolderRoot}><Home size={16} /></button
							>
							{#each folderBreadcrumbs as breadcrumb, index (index)}
								<ChevronRight size={15} class="shrink-0 text-text-muted" />
								<button
									type="button"
									class="max-w-40 truncate rounded px-1 py-0.5 text-text-secondary hover:bg-surface-tertiary hover:text-text-primary"
									onclick={() => goToBreadcrumb(index)}>{breadcrumb}</button
								>
							{/each}
						</nav>
						<div class="flex flex-wrap items-center gap-2">
							{#if info.permissions.download}
								<Button variant="secondary" size="sm" onclick={handleArchiveDownload}
									><Archive size={16} />Download ZIP</Button
								>
							{/if}
							{#if info.permissions.upload}
								<Button
									variant="secondary"
									size="sm"
									disabled={uploading}
									onclick={handleUploadClick}
									><Upload size={16} />{uploading ? 'Uploading...' : 'Add files'}</Button
								>
							{/if}
						</div>
					</div>
					{#if info.permissions.upload}<p class="mt-2 text-xs text-text-muted">
							Uploads are limited to {formatFileSize(info.maxUploadBytes)} per file.
						</p>{/if}

					{#if uploading}<div class="mt-3">
							<ProgressBar value={uploadProgress} showLabel />
						</div>{/if}
					{#if folderError}
						<div class="flex flex-col items-center gap-3 py-16 text-center">
							<p class="m-0 text-sm text-danger">{folderError}</p>
							<Button variant="secondary" size="sm" onclick={() => void loadFolder(folderPath)}
								>Retry</Button
							>
						</div>
					{:else if folderLoading}
						<div class="flex justify-center py-16"><Spinner /></div>
					{:else if folderItems.length === 0}
						<div class="flex flex-col items-center gap-2 py-16 text-center">
							<FolderOpen size={36} class="text-text-muted" />
							<p class="m-0 text-sm text-text-secondary">This folder is empty.</p>
						</div>
					{:else}
						<div class="mt-3 divide-y divide-border-secondary">
							{#each folderItems as item (item.path)}
								<div class="flex items-center gap-3 py-3">
									<button
										type="button"
										class="flex min-w-0 flex-1 items-center gap-3 rounded text-left hover:text-accent"
										onclick={() => openFolderItem(item)}
									>
										<span class="shrink-0 {item.isDir ? 'text-folder' : 'text-accent'}"
											>{#if item.isDir}<FolderOpen size={21} />{:else}<FileText
													size={21}
												/>{/if}</span
										>
										<span class="min-w-0 truncate text-sm font-medium text-text-primary"
											>{item.name}</span
										>
									</button>
									<span class="hidden shrink-0 text-xs text-text-muted sm:inline"
										>{item.isDir ? 'Folder' : formatFileSize(item.size)}</span
									>
									{#if !item.isDir && info.permissions.download}
										<button
											type="button"
											class="rounded p-2 text-text-secondary hover:bg-surface-tertiary hover:text-text-primary"
											title="Download {item.name}"
											aria-label="Download {item.name}"
											onclick={() => handleDownload(item.path)}><Download size={17} /></button
										>
									{:else}
										<button
											type="button"
											class="rounded p-2 text-text-secondary hover:bg-surface-tertiary hover:text-text-primary"
											title="Open {item.name}"
											aria-label="Open {item.name}"
											onclick={() => openFolderItem(item)}><ChevronRight size={17} /></button
										>
									{/if}
									{#if info.permissions.delete}
										<button
											type="button"
											class="rounded p-2 text-text-secondary hover:bg-danger/15 hover:text-danger"
											title="Delete {item.name}"
											aria-label="Delete {item.name}"
											onclick={() => void handleDeleteItem(item)}><Trash2 size={17} /></button
										>
									{/if}
								</div>
							{/each}
						</div>
					{/if}
				</section>
			{:else if info}
				<section class="mx-auto max-w-[720px] p-5 sm:p-8">
					<div class="flex items-center gap-4">
						<span class="shrink-0 text-accent"><TypeIcon size={40} /></span>
						<div class="flex min-w-0 flex-1 items-start justify-between gap-4">
							<div class="min-w-0">
								<h1
									class="m-0 truncate text-lg font-semibold text-text-primary"
									title={info.fileName}
								>
									{info.fileName}
								</h1>
								<p class="m-0 text-sm text-text-secondary">
									{formatFileSize(info.size)} · {getFileTypeDescription(info.fileName)}
								</p>
							</div>
							<p class="m-0 shrink-0 text-right text-xs text-text-muted">
								{#if hasShareExpiry(info.expiresAt)}Expires {formatRelativeTime(
										info.expiresAt
									)}{:else}Never expires{/if}
							</p>
						</div>
					</div>

					{#if info.permissions.view}
						<section class="mt-6">
							{#if previewType === 'image'}
								<img
									src={previewUrl}
									alt={info.fileName}
									class="max-h-[420px] w-full rounded border border-border-primary bg-surface-primary object-contain"
								/>
							{:else if previewType === 'video'}
								<video
									src={previewUrl}
									controls
									preload="metadata"
									playsinline
									class="w-full rounded"
									><track kind="captions" />Your browser does not support the video tag.</video
								>
							{:else if previewType === 'audio'}
								<audio src={previewUrl} controls preload="metadata" class="w-full"
									><track kind="captions" />Your browser does not support the audio tag.</audio
								>
							{:else if previewType === 'pdf'}
								<iframe
									sandbox=""
									src={previewUrl}
									title={info.fileName}
									class="h-[480px] w-full rounded border border-border-primary bg-surface-primary"
								></iframe>
							{:else if isTextPreview}
								{#if textContent !== null}<pre
										class="max-h-[420px] overflow-auto rounded border border-border-primary bg-surface-primary p-4 text-left text-xs break-words whitespace-pre-wrap text-text-primary">{textContent}</pre>{:else if textFailed}<p
										class="m-0 text-sm text-text-secondary"
									>
										Preview unavailable.
									</p>{:else}<div class="flex justify-center py-8"><Spinner /></div>{/if}
							{:else}
								<div
									class="flex justify-center rounded border border-border-primary bg-surface-primary py-12 text-text-muted"
								>
									<TypeIcon size={48} />
								</div>
							{/if}
						</section>
					{/if}
					{#if info.permissions.download}<div class="mt-6">
							<Button variant="primary" onclick={() => handleDownload()}
								><Download size={20} />Download</Button
							>
						</div>{/if}
				</section>
			{/if}
		</div>
	</main>
</div>

<Toast />
