<script lang="ts">
	/**
	 * ImagePreview - Image viewer with zoom support
	 */
	import { ZoomIn, ZoomOut, RotateCw } from 'lucide-svelte';
	import { Button } from '$lib/components/ui';

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

<div class="flex h-full w-full flex-col">
	<!-- Controls -->
	<div
		class="flex shrink-0 items-center justify-center gap-2 border-b border-border-primary bg-surface-secondary p-2"
	>
		<Button variant="secondary" size="icon" onclick={zoomOut} title="Zoom out">
			<ZoomOut size={18} />
		</Button>
		<span class="min-w-[50px] text-center text-xs text-text-secondary"
			>{Math.round(scale * 100)}%</span
		>
		<Button variant="secondary" size="icon" onclick={zoomIn} title="Zoom in">
			<ZoomIn size={18} />
		</Button>
		<Button variant="secondary" size="icon" onclick={rotate} title="Rotate">
			<RotateCw size={18} />
		</Button>
		<Button variant="secondary" size="sm" onclick={resetView}>Reset</Button>
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
				style="--image-scale: {scale}; --image-rotation: {rotation}deg;"
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

<style>
	img {
		transform: scale(var(--image-scale, 1)) rotate(var(--image-rotation, 0deg));
	}
</style>
