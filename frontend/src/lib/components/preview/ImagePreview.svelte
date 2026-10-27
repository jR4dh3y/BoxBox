<script lang="ts">
	/**
	 * ImagePreview - Image viewer with zoom support
	 */
	import { ZoomIn, ZoomOut, RotateCw } from 'lucide-svelte';
	import { Button } from '#lib/components/ui/index.js';

	interface Props {
		url: string;
		filename: string;
	}

	let { url, filename }: Props = $props();

	let scale = $state(1);
	let rotation = $state(0);
	let error = $state<string | null>(null);
	let loading = $state(true);

	function zoomIn() {
		scale = Math.min(scale + 0.25, 5);
	}

	function zoomOut() {
		scale = Math.max(scale - 0.25, 0.25);
	}

	function rotate() {
		rotation = (rotation + 90) % 360;
	}

	function resetView() {
		scale = 1;
		rotation = 0;
	}

	function handleLoad() {
		loading = false;
	}

	function handleError() {
		loading = false;
		error = 'Failed to load image.';
	}

	function handleWheel(event: WheelEvent) {
		event.preventDefault();
		if (event.deltaY < 0) {
			zoomIn();
		} else {
			zoomOut();
		}
	}
</script>

<div class="relative flex h-full w-full flex-col">
	<!-- Controls float over the image so it keeps the whole window -->
	<div
		class="absolute bottom-3 left-1/2 z-10 flex -translate-x-1/2 items-center gap-1 rounded-lg border border-border-primary bg-surface-secondary/95 p-1 shadow-lg"
	>
		<Button variant="ghost" size="icon" onclick={zoomOut} title="Zoom out">
			<ZoomOut size={16} />
		</Button>
		<span class="min-w-[44px] text-center text-xs text-text-secondary"
			>{Math.round(scale * 100)}%</span
		>
		<Button variant="ghost" size="icon" onclick={zoomIn} title="Zoom in">
			<ZoomIn size={16} />
		</Button>
		<Button variant="ghost" size="icon" onclick={rotate} title="Rotate">
			<RotateCw size={16} />
		</Button>
		{#if scale !== 1 || rotation !== 0}
			<Button variant="ghost" size="sm" onclick={resetView}>Reset</Button>
		{/if}
	</div>

	<!-- Image container -->
	<div
		class="flex flex-1 items-center justify-center overflow-auto bg-surface-primary"
		onwheel={handleWheel}
	>
		{#if loading}
			<div class="text-sm text-text-secondary">Loading...</div>
		{/if}
		{#if error}
			<div class="text-sm text-danger">{error}</div>
		{:else}
			<img
				src={url}
				alt={filename}
				style="transform: scale({scale}) rotate({rotation}deg);"
				class="max-h-full max-w-full object-contain transition-transform duration-150 select-none {loading
					? 'opacity-0'
					: ''}"
				onload={handleLoad}
				onerror={handleError}
				draggable="false"
			/>
		{/if}
	</div>
</div>
