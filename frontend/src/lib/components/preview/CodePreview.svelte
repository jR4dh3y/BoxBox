<script lang="ts">
	/**
	 * CodePreview - Code/text viewer with syntax highlighting using Monaco Editor
	 */
	import { onMount, onDestroy } from 'svelte';
	import { getMonacoLanguage } from '$lib/utils/fileTypes';
	import { getFileContent, saveFileContent, type FileInfo } from '$lib/api/files';
	import { Button, Spinner } from '$lib/components/ui';
	import type * as Monaco from 'monaco-editor';

	interface Props {
		url: string;
		filename: string;
		path: string;
		onSaved?: (file: FileInfo) => void;
	}

	let { url, filename, path, onSaved }: Props = $props();

	let containerElement: HTMLDivElement | null = $state(null);
	let editor: Monaco.editor.IStandaloneCodeEditor | null = $state(null);
	let monaco: typeof Monaco | null = $state(null);
	let changeDisposable: Monaco.IDisposable | null = null;
	let content = $state<string | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(true);
	let saving = $state(false);
	let dirty = $state(false);
	let saveError = $state<string | null>(null);
	let saveMessage = $state<string | null>(null);

	const language = $derived(getMonacoLanguage(filename));
	const canSave = $derived(Boolean(editor) && dirty && !loading && !saving && !error);

	function getErrorMessage(value: unknown): string {
		return value instanceof Error ? value.message : 'Failed to save file.';
	}

	function defineTheme(monacoApi: typeof Monaco) {
		monacoApi.editor.defineTheme('filemanager-dark', {
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
	}

	function createEditor() {
		if (!monaco || !containerElement || editor) return;
		const monacoApi = monaco;

		defineTheme(monacoApi);
		const createdEditor = monacoApi.editor.create(containerElement, {
			value: content ?? '',
			language: language,
			theme: 'filemanager-dark',
			readOnly: false,
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
		editor = createdEditor;

		changeDisposable = createdEditor.onDidChangeModelContent(() => {
			dirty = content !== null && createdEditor.getValue() !== content;
			saveMessage = null;
		});
		createdEditor.addCommand(monacoApi.KeyMod.CtrlCmd | monacoApi.KeyCode.KeyS, () => {
			void handleSave();
		});
	}

	async function handleSave() {
		if (!editor || saving || !dirty) return;

		saving = true;
		saveError = null;
		saveMessage = null;

		const nextContent = editor.getValue();
		try {
			const savedFile = await saveFileContent(path, nextContent);
			content = nextContent;
			dirty = false;
			saveMessage = 'Saved';
			onSaved?.(savedFile);
		} catch (saveResult) {
			saveError = getErrorMessage(saveResult);
		} finally {
			saving = false;
		}
	}

	onMount(async () => {
		// Dynamically import Monaco Editor
		try {
			const monacoModule = await import('monaco-editor');
			monaco = monacoModule;

			// Configure Monaco environment for web workers
			self.MonacoEnvironment = {
				getWorker: function () {
					return new Worker(
						URL.createObjectURL(
							new Blob([`self.onmessage = function() {}`], { type: 'text/javascript' })
						)
					);
				}
			};

			createEditor();
		} catch (e) {
			console.error('Failed to load Monaco Editor:', e);
			// Fallback to plain text display
		}
	});

	onDestroy(() => {
		if (changeDisposable) {
			changeDisposable.dispose();
		}
		if (editor) {
			editor.dispose();
		}
	});

	// Load file content whenever preview navigation changes the URL.
	$effect(() => {
		const sourceUrl = url;
		let cancelled = false;

		loading = true;
		error = null;
		content = null;
		dirty = false;
		saveError = null;
		saveMessage = null;

		getFileContent(sourceUrl)
			.then((fileContent) => {
				if (cancelled) return;
				content = fileContent;
			})
			.catch(() => {
				if (cancelled) return;
				error = 'Failed to load file content.';
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

	$effect(() => {
		createEditor();
	});

	// Keep Monaco in sync with loaded content and previewed filename.
	$effect(() => {
		if (!editor || content === null) return;

		editor.setValue(content);
		dirty = false;
		if (monaco) {
			const model = editor.getModel();
			if (model) {
				monaco.editor.setModelLanguage(model, language);
			}
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
	{:else if !editor && content !== null}
		<!-- Fallback: plain text display if Monaco fails to load -->
		<pre
			class="m-0 flex-1 overflow-auto bg-surface-primary p-4 font-mono text-[13px] leading-relaxed break-words whitespace-pre-wrap text-text-primary">{content}</pre>
	{:else}
		<div
			class="flex shrink-0 items-center justify-between gap-3 border-b border-border-primary bg-surface-secondary px-3 py-2"
		>
			<div class="min-w-0 text-xs text-text-secondary">
				<span class="font-medium text-text-primary">Editable</span>
				<span class="ml-2">{dirty ? 'Unsaved changes' : saveMessage || 'No changes'}</span>
			</div>
			<div class="flex items-center gap-3">
				{#if saveError}
					<span class="max-w-80 truncate text-xs text-danger" title={saveError}>{saveError}</span>
				{/if}
				<Button size="sm" variant="secondary" onclick={handleSave} disabled={!canSave}>
					{saving ? 'Saving...' : 'Save'}
				</Button>
			</div>
		</div>
	{/if}
	<div
		bind:this={containerElement}
		class="min-h-0 w-full flex-1 {loading || error || (!editor && content !== null)
			? 'hidden'
			: ''}"
	></div>
</div>
