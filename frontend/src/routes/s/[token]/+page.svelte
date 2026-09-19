<script lang="ts">
	/**
	 * Public share-link recipient page
	 * Standalone design: anyone with the share token can view, download, or
	 * update the shared file without signing in.
	 */
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { AlertTriangle, Download, FolderOpen, Upload } from 'lucide-svelte';
	import { Badge, Button, ProgressBar, Spinner, Toast } from '$lib/components/ui';
	import {
		ApiRequestError,
		getShareInfo,
		hasShareExpiry,
		shareDownloadUrl,
		sharePreviewUrl,
		shareUploadUrl,
		type ShareInfoResponse
	} from '$lib/api';
	import { toastStore } from '$lib/stores/toast.svelte';
	import { getFileIcon, getPreviewType, getFileTypeDescription } from '$lib/utils/fileTypes';
	import { formatFileSize, formatRelativeTime } from '$lib/utils/format';

	// Text previews are fetched in one request; cap them so a huge text file
	// cannot lock up the recipient's browser tab.
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

	const previewType = $derived(info ? getPreviewType(info.fileName) : 'unsupported');
	const previewUrl = $derived(token ? sharePreviewUrl(token) : '');
	const downloadUrl = $derived(token ? shareDownloadUrl(token) : '');
	const isTextPreview = $derived(previewType === 'code' || previewType === 'text');
	const TypeIcon = $derived(info ? getFileIcon(info.fileName, false) : FolderOpen);

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
		try {
			info = await getShareInfo(token);
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

	$effect(() => {
		if (info && isTextPreview && textContent === null && !textFailed) {
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

	function handleDownload() {
		window.open(downloadUrl, '_blank');
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
		uploading = true;
		uploadProgress = 0;
		const xhr = new XMLHttpRequest();
		xhr.open('POST', shareUploadUrl(token));
		xhr.upload.onprogress = (event) => {
			if (event.lengthComputable) {
				uploadProgress = Math.round((event.loaded / event.total) * 100);
			}
		};
		xhr.onload = () => {
			uploading = false;
			if (xhr.status >= 200 && xhr.status < 300) {
				toastStore.success('File updated');
				void loadShare();
			} else if (xhr.status === 403) {
				toastStore.error('This share does not allow updates');
			} else if (xhr.status === 413) {
				toastStore.error('File too large');
			} else if (xhr.status === 404) {
				gone = true;
			} else {
				toastStore.error('Unable to update this file');
			}
		};
		xhr.onerror = () => {
			uploading = false;
			toastStore.error('Unable to update this file');
		};
		xhr.send(file);
	}
</script>

<svelte:head>
	<title>Shared File - BoxBox</title>
</svelte:head>

<input bind:this={uploadInput} type="file" class="hidden" onchange={handleUploadChange} />

<div class="flex min-h-screen items-center justify-center bg-surface-primary p-4">
	<div
		class="w-full max-w-[640px] rounded-lg border border-border-primary bg-surface-secondary p-8 shadow"
	>
		<div class="mb-6 flex items-center gap-3">
			<span class="text-accent"><FolderOpen size={24} /></span>
			<span class="text-lg font-semibold text-text-primary">BoxBox</span>
		</div>

		{#if loading}
			<div class="flex flex-col items-center gap-4 py-16">
				<Spinner size="lg" />
				<span class="text-sm text-text-secondary">Loading share...</span>
			</div>
		{:else if gone}
			<div class="flex flex-col items-center gap-3 py-16 text-center">
				<AlertTriangle size={40} class="text-warning" />
				<h1 class="m-0 text-lg font-semibold text-text-primary">
					This share is no longer available
				</h1>
				<p class="m-0 text-sm text-text-secondary">
					The link may have expired or been revoked by its owner.
				</p>
			</div>
		{:else if loadError}
			<div class="flex flex-col items-center gap-4 py-16 text-center">
				<AlertTriangle size={40} class="text-danger" />
				<p class="m-0 text-sm text-text-secondary">{loadError}</p>
				<Button variant="secondary" size="sm" onclick={() => void loadShare()}>Retry</Button>
			</div>
		{:else if info}
			<div class="flex items-center gap-4">
				<span class="shrink-0 text-accent"><TypeIcon size={40} /></span>
				<div class="min-w-0">
					<h1 class="m-0 truncate text-lg font-semibold text-text-primary" title={info.fileName}>
						{info.fileName}
					</h1>
					<p class="m-0 text-sm text-text-secondary">
						{formatFileSize(info.size)} • {getFileTypeDescription(info.fileName)}
					</p>
				</div>
			</div>

			<div class="mt-4 flex flex-wrap items-center gap-2">
				{#if info.permissions.view}<Badge variant="info">View</Badge>{/if}
				{#if info.permissions.download}<Badge variant="success">Download</Badge>{/if}
				{#if info.permissions.write}<Badge variant="warning">Write</Badge>{/if}
				<span class="text-xs text-text-muted">
					{#if hasShareExpiry(info.expiresAt)}
						Expires {formatRelativeTime(info.expiresAt)}
					{:else}
						Never expires
					{/if}
				</span>
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
						<video src={previewUrl} controls preload="metadata" playsinline class="w-full rounded">
							<track kind="captions" />
							Your browser does not support the video tag.
						</video>
					{:else if previewType === 'audio'}
						<audio src={previewUrl} controls preload="metadata" class="w-full">
							Your browser does not support the audio tag.
						</audio>
					{:else if previewType === 'pdf'}
						<iframe
							sandbox=""
							src={previewUrl}
							title={info.fileName}
							class="h-[480px] w-full rounded border border-border-primary bg-surface-primary"
						></iframe>
					{:else if isTextPreview}
						{#if textContent !== null}
							<pre
								class="max-h-[420px] overflow-auto rounded border border-border-primary bg-surface-primary p-4 text-left text-xs break-words whitespace-pre-wrap text-text-primary">{textContent}</pre>
						{:else if textFailed}
							<p class="m-0 text-sm text-text-secondary">Preview unavailable.</p>
						{:else}
							<div class="flex justify-center py-8"><Spinner /></div>
						{/if}
					{:else}
						<div
							class="flex justify-center rounded border border-border-primary bg-surface-primary py-12 text-text-muted"
						>
							<TypeIcon size={48} />
						</div>
					{/if}
				</section>
			{/if}

			{#if info.permissions.download}
				<div class="mt-6 flex flex-col">
					<Button variant="primary" onclick={handleDownload}>
						<Download size={20} />
						<span>Download</span>
					</Button>
				</div>
			{/if}

			{#if info.permissions.write}
				<section class="mt-6 border-t border-border-secondary pt-6">
					<h2 class="m-0 mb-2 text-sm font-medium text-text-primary">Replace this file</h2>
					<p class="m-0 mb-3 text-xs text-text-secondary">
						The uploaded file replaces the shared file. Its name stays the same.
					</p>
					<div class="flex flex-col gap-3">
						<Button variant="secondary" disabled={uploading} onclick={handleUploadClick}>
							<Upload size={16} />
							<span>{uploading ? 'Uploading...' : 'Choose file'}</span>
						</Button>
						{#if uploading}
							<ProgressBar value={uploadProgress} showLabel />
						{/if}
					</div>
				</section>
			{/if}
		{/if}
	</div>
</div>

<Toast />
