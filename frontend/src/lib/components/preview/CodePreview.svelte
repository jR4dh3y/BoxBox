<script lang="ts">
	/**
	 * CodePreview - Code/text viewer with syntax highlighting using Monaco Editor
	 */
	import { onMount, onDestroy } from 'svelte';
	import { getMonacoLanguage } from '$lib/utils/fileTypes';
	import { getFileContent } from '$lib/api/files';
	import { Spinner } from '$lib/components/ui';
	import type * as MonacoType from 'monaco-editor';

	interface Props {
		url: string;
		filename: string;
	}

	let { url, filename }: Props = $props();

	let containerElement: HTMLDivElement | null = $state(null);
	type MonacoModule = typeof MonacoType;

	let editor = $state<MonacoType.editor.IStandaloneCodeEditor | null>(null);
	let monaco = $state<MonacoModule | null>(null);
	let content = $state<string | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(true);
	let monacoLoadFailed = $state(false);
	let monacoLoading = false;
	let activeRequestId = 0;

	const language = $derived(getMonacoLanguage(filename));

	async function loadMonaco(): Promise<void> {
		if (monaco || monacoLoading || monacoLoadFailed) {
			return;
		}

		monacoLoading = true;

		try {
			const monacoModule = await import('monaco-editor');
			monaco = monacoModule;

			(self as unknown as { MonacoEnvironment?: unknown }).MonacoEnvironment = {
				getWorker(moduleId: string, label: string) {
					void moduleId;
					void label;
					return new Worker(
						URL.createObjectURL(
							new Blob([`self.onmessage = function() {}`], { type: 'text/javascript' })
						)
					);
				}
			};

			monaco.editor.defineTheme('filemanager-dark', {
				base: 'vs-dark',
				inherit: true,
				rules: [],
				colors: {
					'editor.background': '#1e1e1e',
					'editor.foreground': '#d4d4d4',
					'editorLineNumber.foreground': '#5a5a5a',
					'editorLineNumber.activeForeground': '#c6c6c6',
					'editor.selectionBackground': '#264f78',
					'editor.lineHighlightBackground': '#2a2a2a'
				}
			});
		} catch (e) {
			console.error('Failed to load Monaco Editor:', e);
			monacoLoadFailed = true;
		} finally {
			monacoLoading = false;
		}
	}

	function ensureEditor(): void {
		if (!containerElement || !monaco || editor) {
			return;
		}

		editor = monaco.editor.create(containerElement, {
			value: content ?? '',
			language,
			theme: 'filemanager-dark',
			readOnly: true,
			minimap: { enabled: true },
			scrollBeyondLastLine: false,
			fontSize: 13,
			fontFamily: "'Fira Code', 'Cascadia Code', 'JetBrains Mono', Consolas, monospace",
			lineNumbers: 'on',
			renderLineHighlight: 'line',
			automaticLayout: true,
			wordWrap: 'on',
			scrollbar: {
				vertical: 'auto',
				horizontal: 'auto',
				verticalScrollbarSize: 10,
				horizontalScrollbarSize: 10
			}
		});
	}

	async function loadContent(previewUrl: string): Promise<void> {
		const requestId = ++activeRequestId;
		loading = true;
		error = null;

		try {
			const nextContent = await getFileContent(previewUrl);
			if (requestId !== activeRequestId) {
				return;
			}
			content = nextContent;
		} catch {
			if (requestId !== activeRequestId) {
				return;
			}
			error = 'Failed to load file content.';
			content = null;
		} finally {
			if (requestId === activeRequestId) {
				loading = false;
			}
		}
	}

	onMount(() => {
		void loadMonaco();
	});

	onDestroy(() => {
		activeRequestId += 1;
		if (editor) {
			editor.dispose();
			editor = null;
		}
	});

	$effect(() => {
		const previewUrl = url;
		void loadContent(previewUrl);
	});

	$effect(() => {
		ensureEditor();
	});

	$effect(() => {
		if (!editor || content === null) {
			return;
		}

		const model = editor.getModel();
		if (!model) {
			return;
		}

		if (model.getValue() !== content) {
			model.setValue(content);
		}

		if (monaco) {
			monaco.editor.setModelLanguage(model, language);
		}
	});
</script>

<div class="flex h-full w-full flex-col bg-surface-primary">
	{#if loading}
		<div class="flex h-full items-center justify-center">
			<Spinner />
		</div>
	{:else if error}
		<div class="flex h-full items-center justify-center p-5 text-sm text-danger">{error}</div>
	{:else if monacoLoadFailed && content !== null}
		<!-- Fallback: plain text display if Monaco fails to load -->
		<pre
			class="m-0 flex-1 overflow-auto bg-surface-primary p-4 font-mono text-[13px] leading-relaxed break-words whitespace-pre-wrap text-text-primary">{content}</pre>
	{/if}
	<div
		bind:this={containerElement}
		class="min-h-0 w-full flex-1 {loading || error || (monacoLoadFailed && content !== null)
			? 'hidden'
			: ''}"
	></div>
</div>
