<script lang="ts">
	/**
	 * NotebookPreview - Jupyter notebook renderer without Monaco
	 */
	import { AlertTriangle, BookOpen, Image as ImageIcon, Play } from 'lucide-svelte';
	import { getFileContent } from '$lib/api/files';
	import { Spinner } from '$lib/components/ui';

	type NotebookText = string | string[];

	interface NotebookOutput {
		output_type?: string;
		name?: string;
		text?: NotebookText;
		data?: Record<string, NotebookText | undefined>;
		ename?: string;
		evalue?: string;
		traceback?: string[];
	}

	interface NotebookCell {
		id?: string;
		cell_type?: string;
		source?: NotebookText;
		execution_count?: number | null;
		outputs?: NotebookOutput[];
	}

	interface Notebook {
		cells?: NotebookCell[];
		metadata?: Record<string, unknown>;
		nbformat?: number;
		nbformat_minor?: number;
	}

	interface OutputImage {
		mime: string;
		src: string;
	}

	interface Props {
		url: string;
		filename: string;
	}

	let { url, filename }: Props = $props();

	let notebook = $state.raw<Notebook | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	function joinText(value: NotebookText | undefined): string {
		if (!value) return '';
		return Array.isArray(value) ? value.join('') : value;
	}

	function cellSource(cell: NotebookCell): string {
		return joinText(cell.source).trimEnd();
	}

	function cellTypeLabel(cell: NotebookCell): string {
		switch (cell.cell_type) {
			case 'code':
				return 'Code';
			case 'markdown':
				return 'Markdown';
			case 'raw':
				return 'Raw';
			default:
				return cell.cell_type || 'Cell';
		}
	}

	function outputText(output: NotebookOutput): string {
		if (output.output_type === 'error') {
			const traceback = output.traceback?.join('\n') ?? '';
			return [output.ename, output.evalue, traceback].filter(Boolean).join('\n');
		}

		if (output.output_type === 'stream') {
			return joinText(output.text);
		}

		return joinText(output.data?.['text/plain']);
	}

	function outputImages(output: NotebookOutput): OutputImage[] {
		const data = output.data ?? {};
		const images: OutputImage[] = [];

		for (const mime of ['image/png', 'image/jpeg', 'image/gif', 'image/webp']) {
			const encoded = joinText(data[mime]).replace(/\s/g, '');
			if (encoded) {
				images.push({ mime, src: `data:${mime};base64,${encoded}` });
			}
		}

		return images;
	}

	$effect(() => {
		if (!url) return;

		let cancelled = false;
		loading = true;
		error = null;
		notebook = null;

		getFileContent(url)
			.then((content) => {
				if (cancelled) return;
				const parsed = JSON.parse(content) as Notebook;
				if (!Array.isArray(parsed.cells)) {
					throw new Error('Notebook does not contain any cells.');
				}
				notebook = parsed;
			})
			.catch((err: unknown) => {
				if (cancelled) return;
				error = err instanceof Error ? err.message : 'Failed to load notebook preview.';
			})
			.finally(() => {
				if (!cancelled) {
					loading = false;
				}
			});

		return () => {
			cancelled = true;
		};
	});
</script>

<div class="h-full w-full overflow-auto bg-[#f5f7fb] text-[#1f2937]">
	{#if loading}
		<div class="flex h-full items-center justify-center">
			<Spinner />
		</div>
	{:else if error}
		<div class="flex h-full flex-col items-center justify-center gap-3 p-6 text-center">
			<AlertTriangle size={28} class="text-danger" />
			<div>
				<p class="text-sm font-medium">Notebook preview unavailable</p>
				<p class="mt-1 text-sm text-[#667085]">{error}</p>
			</div>
		</div>
	{:else if notebook}
		<div class="mx-auto flex w-full max-w-5xl flex-col gap-3 px-4 py-5">
			<header class="flex items-center gap-3 border-b border-[#d7deea] pb-4">
				<div
					class="flex h-9 w-9 items-center justify-center rounded border border-[#d7deea] bg-white text-[#315f86]"
				>
					<BookOpen size={20} />
				</div>
				<div class="min-w-0">
					<h2 class="truncate text-base leading-6 font-semibold" title={filename}>{filename}</h2>
					<p class="text-xs text-[#667085]">
						{notebook.cells?.length ?? 0} cells
						{#if notebook.nbformat}
							<span> - nbformat {notebook.nbformat}.{notebook.nbformat_minor ?? 0}</span>
						{/if}
					</p>
				</div>
			</header>

			{#each notebook.cells ?? [] as cell, index (cell.id ?? index)}
				{@const source = cellSource(cell)}
				<section class="overflow-hidden rounded border border-[#d7deea] bg-white">
					<div
						class="flex items-center justify-between gap-3 border-b border-[#e6ebf2] bg-[#fbfcff] px-3 py-2"
					>
						<div class="flex min-w-0 items-center gap-2">
							{#if cell.cell_type === 'code'}
								<Play size={14} class="text-[#315f86]" />
							{:else}
								<BookOpen size={14} class="text-[#4d6b59]" />
							{/if}
							<span class="text-xs font-medium text-[#667085] uppercase">
								{cellTypeLabel(cell)}
							</span>
						</div>
						{#if cell.cell_type === 'code'}
							<span class="font-mono text-xs text-[#8a94a6]"
								>In [{cell.execution_count ?? ' '}]:</span
							>
						{/if}
					</div>

					{#if source}
						{#if cell.cell_type === 'markdown'}
							<div class="px-4 py-3 text-sm leading-6 whitespace-pre-wrap">{source}</div>
						{:else}
							<pre
								class="m-0 overflow-auto bg-[#fbfcff] px-4 py-3 font-mono text-[13px] leading-5 text-[#2f3a4a]">{source}</pre>
						{/if}
					{/if}

					{#if cell.outputs?.length}
						<div class="border-t border-[#e6ebf2] bg-[#fcfdff]">
							{#each cell.outputs as output, outputIndex (outputIndex)}
								{@const text = outputText(output)}
								{@const images = outputImages(output)}
								<div class="border-b border-[#e6ebf2] px-4 py-3 last:border-b-0">
									{#if output.output_type === 'error'}
										<div class="mb-2 flex items-center gap-2 text-sm font-medium text-danger">
											<AlertTriangle size={15} />
											<span>Error</span>
										</div>
									{/if}

									{#if images.length}
										<div class="mb-3 flex flex-col gap-3">
											{#each images as image (image.src)}
												<figure class="flex flex-col gap-2">
													<img
														src={image.src}
														alt="Notebook output"
														class="max-h-[520px] max-w-full rounded border border-[#d7deea] bg-white object-contain"
													/>
													<figcaption class="flex items-center gap-1 text-xs text-[#667085]">
														<ImageIcon size={13} />
														{image.mime}
													</figcaption>
												</figure>
											{/each}
										</div>
									{/if}

									{#if text}
										<pre
											class="m-0 overflow-auto rounded bg-[#eef2f7] p-3 font-mono text-[13px] leading-5 break-words whitespace-pre-wrap text-[#1f2937]">{text}</pre>
									{/if}
								</div>
							{/each}
						</div>
					{/if}
				</section>
			{/each}
		</div>
	{/if}
</div>
