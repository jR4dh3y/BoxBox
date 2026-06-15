<script lang="ts">
	/**
	 * PdfPreview - PDF viewer using iframe embed
	 */

	interface Props {
		url: string;
		filename: string;
	}

	let { url, filename }: Props = $props();

	let error = $state<string | null>(null);

	function handleError() {
		error = 'Failed to load PDF. Try downloading the file instead.';
	}
</script>

<div class="flex h-full w-full items-center justify-center bg-surface-tertiary">
	{#if error}
		<div class="p-5 text-center text-sm text-danger">{error}</div>
	{:else}
		<iframe
			src={url}
			title={filename}
			onerror={handleError}
			class="h-full w-full border-none bg-white"
		></iframe>
	{/if}
</div>
