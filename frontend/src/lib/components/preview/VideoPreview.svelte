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

	let { url, downloadUrl }: Props = $props();

	let failedUrl = $state<string | null>(null);
	let errorMessage = $state<string | null>(null);

	const currentError = $derived(failedUrl === url ? errorMessage : null);

	function setError(message: string) {
		failedUrl = url;
		errorMessage = message;
	}

	function handleError(event: Event) {
		const mediaElement =
			event.currentTarget instanceof HTMLMediaElement ? event.currentTarget : null;
		const mediaError = mediaElement?.error;
		if (mediaError?.code === MediaError.MEDIA_ERR_DECODE) {
			setError(
				'This video file uses a codec your browser cannot decode. Download it or play it in a native media player.'
			);
			return;
		}
		if (mediaError?.code === MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED) {
			setError(
				'This video format is not supported by your browser. Download it or play it in a native media player.'
			);
			return;
		}
		setError(
			'Failed to load video. The file may be corrupted or the codec is not supported by your browser.'
		);
	}

	function openDownload() {
		if (downloadUrl) {
			window.open(downloadUrl, '_blank');
		}
	}
</script>

<div class="flex h-full w-full items-center justify-center bg-black">
	{#if currentError}
		<div class="flex max-w-2xl flex-col items-center gap-4 p-5 text-center text-sm text-danger">
			<p>{currentError}</p>
			{#if downloadUrl}
				<Button variant="primary" onclick={openDownload}>
					<Download size={18} />
					Download Video
				</Button>
			{/if}
		</div>
	{:else}
		{#key url}
			<video
				src={url}
				controls
				preload="metadata"
				playsinline
				onerror={handleError}
				class="max-h-full max-w-full outline-none"
			>
				<track kind="captions" />
				Your browser does not support the video tag.
			</video>
		{/key}
	{/if}
</div>
