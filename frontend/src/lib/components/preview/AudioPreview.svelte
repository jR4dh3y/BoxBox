<script lang="ts">
	/**
	 * AudioPreview - HTML5 audio player with visualization
	 */
	import { Music } from 'lucide-svelte';

	interface Props {
		url: string;
		filename: string;
	}

	let { url, filename }: Props = $props();

	let error = $state<string | null>(null);

	function handleError() {
		error = 'Failed to load audio. The format may not be supported by your browser.';
	}
</script>

<div class="flex w-full max-w-md flex-col items-center justify-center gap-5 p-10">
	<div class="text-accent opacity-80">
		<Music size={64} />
	</div>
	<div class="text-center text-base font-medium break-words text-text-primary">{filename}</div>
	{#if error}
		<div class="text-center text-sm text-danger">{error}</div>
	{:else}
		<audio
			src={url}
			controls
			autoplay
			preload="metadata"
			onerror={handleError}
			class="w-full outline-none"
		>
			Your browser does not support the audio tag.
		</audio>
	{/if}
</div>
