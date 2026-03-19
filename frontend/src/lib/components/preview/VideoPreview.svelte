<script lang="ts">
	/**
	 * VideoPreview - HTML5 video player with streaming support
	 */
	import { Button } from '$lib/components/ui';
	import { Download } from 'lucide-svelte';

	interface Props {
		url: string;
		filename: string;
		downloadUrl?: string;
		sizeBytes?: number;
	}

	let { url, filename, downloadUrl, sizeBytes = 0 }: Props = $props();

	let videoElement: HTMLVideoElement | null = $state(null);
	let startedPreviewFor: string | null = $state(null);
	let errorMessage: string | null = $state(null);
	let errorUrl: string | null = $state(null);
	const LARGE_VIDEO_THRESHOLD_BYTES = 500 * 1024 * 1024;
	const isLargeVideo = $derived(sizeBytes >= LARGE_VIDEO_THRESHOLD_BYTES);
	const shouldLoadVideo = $derived(!isLargeVideo || startedPreviewFor === url);
	const error = $derived(errorUrl === url ? errorMessage : null);

	function handleError() {
		const mediaError = videoElement?.error;
		if (mediaError?.code === MediaError.MEDIA_ERR_DECODE) {
			errorMessage =
				'This video file uses a codec your browser cannot decode. Download it or play it in a native media player.';
			errorUrl = url;
			return;
		}
		if (mediaError?.code === MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED) {
			errorMessage =
				'This video format is not supported by your browser. Download it or play it in a native media player.';
			errorUrl = url;
			return;
		}
		errorMessage =
			'Failed to load video. The file may be corrupted or the codec is not supported by your browser.';
		errorUrl = url;
	}

	function openDownload() {
		if (downloadUrl) {
			window.open(downloadUrl, '_blank');
		}
	}

	function startPreview() {
		startedPreviewFor = url;
	}
</script>

<div class="flex h-full w-full items-center justify-center bg-black">
	{#if error}
		<div class="flex max-w-2xl flex-col items-center gap-4 p-5 text-center text-sm text-danger">
			<p>{error}</p>
			{#if downloadUrl}
				<Button variant="primary" onclick={openDownload}>
					<Download size={18} />
					Download Video
				</Button>
			{/if}
		</div>
	{:else if !shouldLoadVideo}
		<div class="flex max-w-2xl flex-col items-center gap-4 p-5 text-center text-sm">
			<p class="text-text-secondary">
				This is a large video file. Click below to start streaming preview.
			</p>
			<Button variant="primary" onclick={startPreview}>Start Preview</Button>
			{#if downloadUrl}
				<Button variant="secondary" onclick={openDownload}>
					<Download size={18} />
					Download Video
				</Button>
			{/if}
		</div>
	{:else}
		{#key url}
			<video
				bind:this={videoElement}
				src={url}
				controls
				preload={isLargeVideo ? 'none' : 'metadata'}
				playsinline
				aria-label={filename}
				onerror={handleError}
				class="max-h-full max-w-full outline-none"
			>
				<track kind="captions" />
				Your browser does not support the video tag.
			</video>
		{/key}
	{/if}
</div>
