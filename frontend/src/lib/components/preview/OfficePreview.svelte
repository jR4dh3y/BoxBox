<script lang="ts">
	/**
	 * OfficePreview - browser-only previews for supported Office formats
	 */
	import { AlertTriangle, Download, Sheet, Table2 } from 'lucide-svelte';
	import { Button, Spinner } from '$lib/components/ui';
	import { getExtension } from '$lib/utils/fileTypes';

	type PreviewMode = 'docx' | 'sheet' | 'pptx' | 'unsupported';
	type LoadStatus = 'idle' | 'loading' | 'ready' | 'error';
	type SheetRow = Array<string | number | boolean | Date | null>;

	interface SheetPreview {
		name: string;
		rows: SheetRow[];
	}

	interface Props {
		url: string;
		filename: string;
		downloadUrl: string;
	}

	let { url, filename, downloadUrl }: Props = $props();

	let docxContainer: HTMLDivElement | null = $state(null);
	let pptxContainer: HTMLDivElement | null = $state(null);
	let mode = $state<PreviewMode>('unsupported');
	let status = $state<LoadStatus>('idle');
	let error = $state<string | null>(null);
	let sheets = $state.raw<SheetPreview[]>([]);
	let selectedSheet = $state(0);

	const ext = $derived(getExtension(filename));
	const visibleRows = $derived(sheets[selectedSheet]?.rows.slice(0, 250) ?? []);
	const maxColumns = $derived(visibleRows.reduce((max, row) => Math.max(max, row.length), 0));
	const activeSheetName = $derived(sheets[selectedSheet]?.name ?? '');
	const rowLimitReached = $derived((sheets[selectedSheet]?.rows.length ?? 0) > visibleRows.length);

	function detectMode(extension: string): PreviewMode {
		if (['docx', 'docm', 'dotx', 'dotm'].includes(extension)) return 'docx';
		if (['xlsx', 'xls', 'xlsm', 'xlsb', 'xltx', 'xltm', 'ods', 'ots'].includes(extension)) {
			return 'sheet';
		}
		if (['pptx', 'pptm', 'ppsx', 'ppsm', 'potx', 'potm'].includes(extension)) return 'pptx';
		return 'unsupported';
	}

	function openDownload() {
		window.open(downloadUrl, '_blank');
	}

	function formatCell(value: SheetRow[number]): string {
		if (value === null || value === undefined) return '';
		if (value instanceof Date) return value.toLocaleString();
		return String(value);
	}

	function resetPreview(nextMode: PreviewMode) {
		mode = nextMode;
		status = nextMode === 'unsupported' ? 'error' : 'loading';
		error =
			nextMode === 'unsupported'
				? `${ext.toUpperCase()} files do not have a reliable browser-only renderer here yet.`
				: null;
		sheets = [];
		selectedSheet = 0;
		if (docxContainer) docxContainer.replaceChildren();
		if (pptxContainer) pptxContainer.replaceChildren();
	}

	async function fetchBuffer(signal: AbortSignal): Promise<ArrayBuffer> {
		const response = await fetch(url, { signal });
		if (!response.ok) {
			throw new Error(`Failed to fetch file: ${response.statusText}`);
		}
		return response.arrayBuffer();
	}

	async function renderDocx(buffer: ArrayBuffer) {
		if (!docxContainer) {
			throw new Error('Document preview container is not ready.');
		}

		const { renderAsync } = await import('docx-preview');
		docxContainer.replaceChildren();
		await renderAsync(buffer, docxContainer, undefined, {
			breakPages: true,
			experimental: true,
			renderChanges: false,
			renderComments: false,
			renderAltChunks: false,
			useBase64URL: true
		});
	}

	async function renderSheet(buffer: ArrayBuffer) {
		const xlsx = await import('xlsx');
		const workbook = xlsx.read(buffer, {
			type: 'array',
			cellDates: true,
			cellNF: false,
			cellStyles: false
		});

		sheets = workbook.SheetNames.map((name) => {
			const worksheet = workbook.Sheets[name];
			const rows = xlsx.utils.sheet_to_json<SheetRow>(worksheet, {
				header: 1,
				defval: '',
				blankrows: false,
				raw: false
			});
			return { name, rows };
		});

		if (sheets.length === 0) {
			throw new Error('Workbook does not contain any visible sheets.');
		}
	}

	async function renderPptx(buffer: ArrayBuffer) {
		if (!pptxContainer) {
			throw new Error('Presentation preview container is not ready.');
		}

		const { init } = await import('pptx-preview');
		pptxContainer.replaceChildren();
		const previewer = init(pptxContainer, {
			width: 960,
			height: 540
		});
		await previewer.preview(buffer);
	}

	$effect(() => {
		if (!url) return;

		const nextMode = detectMode(ext);
		resetPreview(nextMode);
		if (nextMode === 'unsupported') return;

		const controller = new AbortController();

		fetchBuffer(controller.signal)
			.then(async (buffer) => {
				if (controller.signal.aborted) return;
				if (nextMode === 'docx') {
					await renderDocx(buffer);
				} else if (nextMode === 'sheet') {
					await renderSheet(buffer);
				} else if (nextMode === 'pptx') {
					await renderPptx(buffer);
				}
				if (!controller.signal.aborted) {
					status = 'ready';
				}
			})
			.catch((err: unknown) => {
				if (controller.signal.aborted) return;
				error = err instanceof Error ? err.message : 'Failed to render Office preview.';
				status = 'error';
			});

		return () => {
			controller.abort();
		};
	});
</script>

<div class="flex h-full w-full flex-col bg-surface-primary">
	{#if status === 'loading'}
		<div class="flex h-full flex-col items-center justify-center gap-3 text-text-secondary">
			<Spinner />
			<span class="text-sm">Rendering locally...</span>
		</div>
	{/if}

	{#if status === 'error'}
		<div class="flex h-full flex-col items-center justify-center gap-4 p-6 text-center">
			<div
				class="flex h-12 w-12 items-center justify-center rounded border border-border-primary bg-surface-secondary text-text-secondary"
			>
				<AlertTriangle size={24} />
			</div>
			<div class="max-w-md space-y-1">
				<p class="text-sm font-medium text-text-primary">Local preview unavailable</p>
				<p class="text-sm text-text-secondary">{error}</p>
			</div>
			<Button variant="primary" onclick={openDownload}>
				<Download size={18} />
				Download File
			</Button>
		</div>
	{/if}

	{#if mode === 'docx' && status !== 'error'}
		<div class="h-full overflow-auto bg-[#eef1f5]">
			<div bind:this={docxContainer} class="docx-preview-host mx-auto min-h-full p-5"></div>
		</div>
	{/if}

	{#if mode === 'sheet' && status !== 'error'}
		<div class="flex h-full min-h-0 flex-col bg-[#f6f8fb]">
			<div
				class="flex shrink-0 items-center justify-between gap-3 border-b border-border-primary bg-surface-secondary px-3 py-2"
			>
				<div class="flex min-w-0 items-center gap-2">
					<Sheet size={16} class="text-accent" />
					<span class="truncate text-sm font-medium text-text-primary" title={activeSheetName}>
						{activeSheetName || 'Workbook'}
					</span>
				</div>
				{#if rowLimitReached}
					<span class="shrink-0 text-xs text-text-secondary"
						>Showing first {visibleRows.length} rows</span
					>
				{/if}
			</div>

			{#if sheets.length > 1}
				<div
					class="flex shrink-0 gap-1 overflow-x-auto border-b border-border-secondary bg-surface-primary px-2 py-1.5"
				>
					{#each sheets as sheet, index (sheet.name)}
						<button
							type="button"
							class="rounded px-2 py-1 text-xs {selectedSheet === index
								? 'bg-accent text-white'
								: 'text-text-secondary hover:bg-surface-secondary hover:text-text-primary'}"
							onclick={() => (selectedSheet = index)}
							title={sheet.name}
						>
							{sheet.name}
						</button>
					{/each}
				</div>
			{/if}

			<div class="min-h-0 flex-1 overflow-auto">
				{#if visibleRows.length === 0}
					<div class="flex h-full flex-col items-center justify-center gap-2 text-text-secondary">
						<Table2 size={28} />
						<span class="text-sm">This sheet is empty</span>
					</div>
				{:else}
					<table class="min-w-full border-collapse bg-white text-left text-[13px] text-[#1f2937]">
						<tbody>
							{#each visibleRows as row, rowIndex (rowIndex)}
								<tr class={rowIndex === 0 ? 'bg-[#eef4ff]' : 'odd:bg-white even:bg-[#fafbfc]'}>
									<th
										class="sticky left-0 z-[1] w-12 border border-[#d8dee8] bg-[#f1f4f8] px-2 py-1 text-right font-mono text-[11px] text-[#667085]"
									>
										{rowIndex + 1}
									</th>
									{#each Array.from({ length: maxColumns }) as _, columnIndex (columnIndex)}
										<td
											class="max-w-[360px] min-w-[120px] border border-[#d8dee8] px-2 py-1 align-top whitespace-pre-wrap"
											title={formatCell(row[columnIndex])}
										>
											{formatCell(row[columnIndex])}
										</td>
									{/each}
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</div>
		</div>
	{/if}

	{#if mode === 'pptx' && status !== 'error'}
		<div class="h-full overflow-auto bg-[#111827] p-5">
			<div class="flex min-h-full items-start justify-center">
				<div bind:this={pptxContainer} class="pptx-preview-host"></div>
			</div>
		</div>
	{/if}
</div>

<style>
	.docx-preview-host :global(.docx-wrapper) {
		background: transparent;
		padding: 0;
	}

	.docx-preview-host :global(section.docx) {
		margin: 0 auto 16px;
		box-shadow: 0 10px 30px rgb(15 23 42 / 0.18);
	}

	.pptx-preview-host :global(canvas),
	.pptx-preview-host :global(svg),
	.pptx-preview-host :global(.slide) {
		max-width: 100%;
	}
</style>
