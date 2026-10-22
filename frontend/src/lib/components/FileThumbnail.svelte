<script lang="ts">
	import type { FileInfo } from '#lib/api/files.js';
	import { getThumbnailUrl } from '#lib/api/files.js';
	import { getThumbnailSourceKey, supportsThumbnailFile } from '#lib/utils/thumbnail.js';
	import { getFileIcon } from '#lib/utils/fileTypes.js';

	let {
		file,
		width = 224,
		height = 144,
		alt = undefined,
		class: className = ''
	}: {
		file: FileInfo;
		width?: number;
		height?: number;
		alt?: string;
		class?: string;
	} = $props();

	const sourceKey = $derived(getThumbnailSourceKey(file));
	const imageUrl = $derived(getThumbnailUrl(file.path, width, height, sourceKey));
	const isSupported = $derived(supportsThumbnailFile(file));
	const imageAlt = $derived(alt ?? file.name);
	const IconComponent = $derived(getFileIcon(file.name, file.isDir));
	let failedKey = $state<string | null>(null);
	let loadedKey = $state<string | null>(null);
	const showImage = $derived(isSupported && failedKey !== sourceKey);
	const isLoading = $derived(showImage && loadedKey !== sourceKey);
	const showFallback = $derived(!showImage || isLoading);
	const showFallbackLabel = $derived(!isSupported || failedKey === sourceKey);

	function handleLoad() {
		loadedKey = sourceKey;
	}

	function handleError() {
		failedKey = sourceKey;
	}
</script>

<div
	class="relative flex w-full items-center justify-center overflow-hidden {className}"
	style:aspect-ratio={`${width} / ${height}`}
	aria-busy={isLoading ? 'true' : undefined}
	role={showFallbackLabel && imageAlt ? 'img' : undefined}
	aria-label={showFallbackLabel ? imageAlt : undefined}
>
	{#if showFallback}
		<IconComponent
			size={Math.min(32, Math.max(16, width * 0.2))}
			class={file.isDir ? 'text-folder' : 'text-text-secondary'}
			aria-hidden="true"
		/>
	{/if}
	{#if showImage}
		<img
			src={imageUrl}
			alt={imageAlt}
			{width}
			{height}
			loading="lazy"
			decoding="async"
			class={[
				'absolute inset-0 h-full w-full object-contain',
				loadedKey === sourceKey ? 'opacity-100' : 'opacity-0'
			]}
			onload={handleLoad}
			onerror={handleError}
		/>
	{/if}
</div>
