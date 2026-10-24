<script lang="ts">
	import { Folder, Home, Loader2 } from 'lucide-svelte';
	import { onDestroy, tick } from 'svelte';
	import { suggestedDirectories } from '#lib/browse/pathSuggestions.js';
	import { listDirectory, listRoots } from '#lib/api/files.js';
	import { isLocalDesktop } from '#lib/api/desktop.js';
	import { desktopInputPath } from '#lib/browse/desktopPath.js';

	let {
		pathSegments = [],
		roots = [],
		pageTitle = null,
		onNavigate,
		includeHiddenSuggestions = false
	}: {
		pathSegments?: string[];
		roots?: MountPoint[];
		/** Shown instead of the path while the pane shows a page rather than a folder. */
		pageTitle?: string | null;
		onNavigate?: (path: string) => void;
		includeHiddenSuggestions?: boolean;
	} = $props();

	const breadcrumbs = $derived.by(() => {
		const path = pathSegments.join('/');
		const root = roots.find((root) => path === root.name || path.startsWith(`${root.name}/`));
		if (root) {
			const label = root.serverName
				? `${root.serverName} · ${root.label ?? root.name}`
				: (root.label ?? root.name);
			const suffix = path.slice(root.name.length).split('/').filter(Boolean);
			return [
				{ label, path: root.name },
				...suffix.map((label, index) => ({
					label,
					path: `${root.name}/${suffix.slice(0, index + 1).join('/')}`
				}))
			];
		}
		return pathSegments[0] === '@remote'
			? pathSegments
					.slice(2)
					.map((label, index) => ({ label, path: pathSegments.slice(0, index + 3).join('/') }))
			: pathSegments.map((label, index) => ({
					label,
					path: pathSegments.slice(0, index + 1).join('/')
				}));
	});
	let editing = $state(false);
	let pathBar = $state<HTMLDivElement>();

	/** Open the location editor; it focuses and selects the path itself. */
	export function edit(): void {
		editing = true;
	}

	function closeEditor(): void {
		editing = false;
		// Keep focus the user moved elsewhere; reclaim it only when the editor took it with it.
		void tick().then(() => {
			if (!document.activeElement || document.activeElement === document.body) pathBar?.focus();
		});
	}

	let { pathSegments = [], onNavigate, includeHiddenSuggestions = false }: Props = $props();

	const suggestionListId = 'toolbar-path-suggestions';

	let isEditingPath = $state(false);
	let pathDraft = $state('');
	let suggestions = $state<PathSuggestion[]>([]);
	let activeSuggestionIndex = $state(0);
	let suggestionSelectedByKeyboard = $state(false);
	let isLoadingSuggestions = $state(false);
	let pathSelectionStart = $state(0);
	let pathSelectionEnd = $state(0);
	let pathInputEl = $state<HTMLInputElement | undefined>();
	let suggestionListEl = $state<HTMLDivElement | undefined>();
	let suggestionTimer: ReturnType<typeof setTimeout> | undefined;
	let blurTimer: ReturnType<typeof setTimeout> | undefined;
	let suggestionRequestId = 0;

	const currentPath = $derived(pathSegments.join('/'));
	const activeSuggestion = $derived(suggestions[activeSuggestionIndex] ?? null);
	const completionSuffix = $derived.by(() => {
		if (!isEditingPath || !activeSuggestion) return '';

		const normalizedDraft = normalizeInputPath(pathDraft).toLocaleLowerCase();
		const suggestionPath = activeSuggestion.path;

		if (!suggestionPath.toLocaleLowerCase().startsWith(normalizedDraft)) return '';
		return suggestionPath.slice(normalizedDraft.length);
	});
	const hasPathSelection = $derived(pathSelectionStart !== pathSelectionEnd);
	const pathSelectionMin = $derived(Math.min(pathSelectionStart, pathSelectionEnd));
	const pathSelectionMax = $derived(Math.max(pathSelectionStart, pathSelectionEnd));
	const draftDisplayParts = $derived.by(() =>
		buildPathDisplayParts(pathDraft, pathSelectionMin, pathSelectionMax, pathSelectionStart)
	);
	const completionDisplayText = $derived(
		!hasPathSelection && pathDraft.length > 0 ? formatPathText(completionSuffix) : ''
	);
	const showSuggestionPopup = $derived(
		isEditingPath && (suggestions.length > 0 || isLoadingSuggestions)
	);
	const activeSuggestionOptionId = $derived(
		showSuggestionPopup && activeSuggestion
			? `${suggestionListId}-${activeSuggestionIndex}`
			: undefined
	);

	onDestroy(() => {
		clearSuggestionTimer();
		clearBlurTimer();
		suggestionRequestId++;
	});

	function buildPath(index: number): string {
		return pathSegments.slice(0, index + 1).join('/');
	}

	function handleSegmentClick(index: number) {
		onNavigate?.(buildPath(index));
	}

	function handleRootClick() {
		onNavigate?.('');
	}

	function normalizeInputPath(value: string): string {
		let path = value;
		if (isLocalDesktop()) {
			try {
				path = desktopInputPath(value);
			} catch {
				return '';
			}
		}
		return path
			.replaceAll('\\', '/')
			.replace(/^\/+/, '')
			.replace(/\/{2,}/g, '/');
	}

	function normalizeNavigablePath(value: string): string {
		return normalizeInputPath(value).replace(/\/+$/, '').trim();
	}

	function formatPathText(value: string): string {
		return value
			.replaceAll('\\', '/')
			.replace(/\/{2,}/g, '/')
			.replaceAll('/', ' / ');
	}

	function pushDisplayPart(parts: PathDisplayPart[], part: PathDisplayPart): void {
		const previous = parts.at(-1);
		if (!part.caret && previous && !previous.caret && previous.selected === part.selected) {
			previous.text += part.text;
			return;
		}

		parts.push(part);
	}

	function buildPathDisplayParts(
		value: string,
		selectionMin: number,
		selectionMax: number,
		caretOffset: number
	): PathDisplayPart[] {
		const normalized = normalizeInputPath(value);
		const parts: PathDisplayPart[] = [];

		for (let offset = 0; offset <= normalized.length; offset += 1) {
			if (selectionMin === selectionMax && offset === caretOffset) {
				pushDisplayPart(parts, { text: '', selected: false, caret: true });
			}

			if (offset === normalized.length) break;

			pushDisplayPart(parts, {
				text: normalized[offset] === '/' ? ' / ' : normalized[offset],
				selected: offset >= selectionMin && offset < selectionMax
			});
		}

		return parts;
	}

	function syncPathSelection(): void {
		if (!pathInputEl) return;

		pathSelectionStart = pathInputEl.selectionStart ?? pathDraft.length;
		pathSelectionEnd = pathInputEl.selectionEnd ?? pathSelectionStart;
	}

	function scrollActiveSuggestionIntoView(): void {
		const activeOption = suggestionListEl?.querySelector<HTMLElement>(
			`[data-suggestion-index="${activeSuggestionIndex}"]`
		);

		activeOption?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
	}

	function queueActiveSuggestionScroll(): void {
		void tick().then(scrollActiveSuggestionIntoView);
	}

	function resetSuggestionScroll(): void {
		void tick().then(() => {
			if (suggestionListEl) {
				suggestionListEl.scrollTop = 0;
			}
		});
	}

	function getSuggestionParts(value: string): { parentPath: string; prefix: string } {
		const normalized = normalizeInputPath(value);
		const withoutTrailingSlash = normalized.replace(/\/+$/, '');

		if (normalized.endsWith('/')) {
			return { parentPath: withoutTrailingSlash, prefix: '' };
		}

		const lastSlashIndex = withoutTrailingSlash.lastIndexOf('/');
		if (lastSlashIndex === -1) {
			return { parentPath: '', prefix: withoutTrailingSlash };
		}

		return {
			parentPath: withoutTrailingSlash.slice(0, lastSlashIndex),
			prefix: withoutTrailingSlash.slice(lastSlashIndex + 1)
		};
	}

	function buildSuggestionPath(parentPath: string, name: string): string {
		return parentPath ? `${parentPath}/${name}` : name;
	}

	function filterSuggestions(
		names: Array<{ name: string }>,
		parentPath: string,
		prefix: string
	): PathSuggestion[] {
		const normalizedPrefix = prefix.toLocaleLowerCase();

		return names
			.filter((item) => item.name.toLocaleLowerCase().startsWith(normalizedPrefix))
			.map((item) => ({
				path: buildSuggestionPath(parentPath, item.name)
			}));
	}

	async function fetchRootSuggestions(prefix: string): Promise<PathSuggestion[]> {
		const response = await listRoots();
		return filterSuggestions(response.roots, '', prefix);
	}

	async function fetchChildSuggestions(
		parentPath: string,
		prefix: string
	): Promise<PathSuggestion[]> {
		const response = await listDirectory(parentPath);
		const directories = suggestedDirectories(response.items, prefix, includeHiddenSuggestions);
		return filterSuggestions(directories, parentPath, prefix);
	}

	async function loadSuggestions(value: string): Promise<void> {
		const requestId = ++suggestionRequestId;
		const { parentPath, prefix } = getSuggestionParts(value);

		isLoadingSuggestions = true;

		try {
			const nextSuggestions = parentPath
				? await fetchChildSuggestions(parentPath, prefix)
				: await fetchRootSuggestions(prefix);

			if (requestId !== suggestionRequestId) return;

			suggestions = nextSuggestions;
			activeSuggestionIndex = 0;
			suggestionSelectedByKeyboard = false;
			resetSuggestionScroll();
		} catch {
			if (requestId !== suggestionRequestId) return;
			suggestions = [];
			activeSuggestionIndex = 0;
			suggestionSelectedByKeyboard = false;
			resetSuggestionScroll();
		} finally {
			if (requestId === suggestionRequestId) {
				isLoadingSuggestions = false;
			}
		}
	}

	function queueSuggestions(): void {
		clearSuggestionTimer();

		if (!isEditingPath) return;

		const value = pathDraft;
		suggestionTimer = setTimeout(() => {
			suggestionTimer = undefined;
			void loadSuggestions(value);
		}, 120);
	}

	async function beginPathEdit(): Promise<void> {
		if (isEditingPath) return;

		isEditingPath = true;
		pathDraft = currentPath;
		pathSelectionStart = 0;
		pathSelectionEnd = currentPath.length;
		suggestions = [];
		activeSuggestionIndex = 0;
		suggestionSelectedByKeyboard = false;
		queueSuggestions();

		await tick();

		pathInputEl?.focus();
		pathInputEl?.select();
		syncPathSelection();
	}

	function cancelPathEdit(): void {
		clearSuggestionTimer();
		clearBlurTimer();

		isEditingPath = false;
		pathDraft = '';
		pathSelectionStart = 0;
		pathSelectionEnd = 0;
		suggestions = [];
		activeSuggestionIndex = 0;
		suggestionSelectedByKeyboard = false;
		isLoadingSuggestions = false;
		suggestionRequestId++;
	}

	function commitPath(value: string = pathDraft): void {
		const nextPath = normalizeNavigablePath(value);
		cancelPathEdit();

		if (nextPath !== currentPath) {
			onNavigate?.(nextPath);
		}
	}

	function completePath(value: string): void {
		pathDraft = `${value}/`;
		activeSuggestionIndex = 0;
		suggestionSelectedByKeyboard = false;
		queueSuggestions();

		void tick().then(() => {
			const cursorPosition = pathDraft.length;
			pathInputEl?.focus();
			pathInputEl?.setSelectionRange(cursorPosition, cursorPosition);
			syncPathSelection();
		});
	}

	function handlePathBarClick(event: MouseEvent): void {
		const target = event.target as HTMLElement;
		if (target.closest('button') || isEditingPath) return;

		void beginPathEdit();
	}

	function handlePathBarKeydown(event: KeyboardEvent): void {
		const target = event.target as HTMLElement;
		if (target.closest('button, input') || isEditingPath) return;
		if (event.key !== 'Enter' && event.key !== 'F2') return;

		event.preventDefault();
		void beginPathEdit();
	}

	function handlePathInput(event: Event): void {
		pathDraft = normalizeInputPath((event.target as HTMLInputElement).value);
		activeSuggestionIndex = 0;
		suggestionSelectedByKeyboard = false;
		queueSuggestions();

		void tick().then(syncPathSelection);
	}

	function handlePathInputFocus(): void {
		clearBlurTimer();

		syncPathSelection();
	}

	function handlePathInputBlur(): void {
		clearBlurTimer();
		blurTimer = setTimeout(() => {
			blurTimer = undefined;
			if (isEditingPath) {
				cancelPathEdit();
			}
		}, 120);
	}

	function handlePathSubmit(event: SubmitEvent): void {
		event.preventDefault();

		const shouldUseSuggestion =
			Boolean(activeSuggestion) &&
			(suggestionSelectedByKeyboard || (pathDraft.length > 0 && !pathDraft.endsWith('/')));

		commitPath(shouldUseSuggestion ? activeSuggestion?.path : pathDraft);
	}

	function isCaretAtEnd(): boolean {
		return (
			pathInputEl?.selectionStart === pathDraft.length &&
			pathInputEl?.selectionEnd === pathDraft.length
		);
	}

	function handlePathKeydown(event: KeyboardEvent): void {
		switch (event.key) {
			case 'Escape':
				event.preventDefault();
				cancelPathEdit();
				break;

			case 'ArrowDown':
				if (suggestions.length === 0) return;
				event.preventDefault();
				activeSuggestionIndex = (activeSuggestionIndex + 1) % suggestions.length;
				suggestionSelectedByKeyboard = true;
				queueActiveSuggestionScroll();
				break;

			case 'ArrowUp':
				if (suggestions.length === 0) return;
				event.preventDefault();
				activeSuggestionIndex =
					(activeSuggestionIndex - 1 + suggestions.length) % suggestions.length;
				suggestionSelectedByKeyboard = true;
				queueActiveSuggestionScroll();
				break;

			case 'Tab':
				if (!activeSuggestion) return;
				event.preventDefault();
				completePath(activeSuggestion.path);
				break;

			case 'ArrowRight':
				if (!activeSuggestion || !completionSuffix || !isCaretAtEnd()) return;
				event.preventDefault();
				completePath(activeSuggestion.path);
				break;
		}
	}

	function handleSuggestionPointerDown(event: PointerEvent, suggestion: PathSuggestion): void {
		event.preventDefault();
		commitPath(suggestion.path);
	}

	function clearSuggestionTimer(): void {
		if (!suggestionTimer) return;

		clearTimeout(suggestionTimer);
		suggestionTimer = undefined;
	}

	function clearBlurTimer(): void {
		if (!blurTimer) return;

		clearTimeout(blurTimer);
		blurTimer = undefined;
	}
</script>

<svelte:window
	onkeydown={(event) => {
		if (editing && event.key === 'Escape') {
			event.preventDefault();
			closeEditor();
		}
	}}
/>

{#if editing}
	{#await import('./PathEditor.svelte')}
		<div class="flex min-w-0 flex-1 items-center px-2 py-1 text-sm text-text-muted" role="status">
			Opening location editor…
		</div>
	{:then { default: PathEditor }}
		<PathEditor
			{pathSegments}
			{roots}
			{onNavigate}
			{includeHiddenSuggestions}
			onClose={closeEditor}
		/>
	{:catch}
		<LoadError label="location editor" onClose={closeEditor} />
	{/await}
{:else}
	<div
		bind:this={pathBar}
		class="relative flex min-w-0 flex-1 cursor-text items-center gap-1.5 rounded border border-border-primary bg-surface-secondary px-2 py-1 focus-within:border-border-focus"
		role="button"
		tabindex="0"
		aria-label="Edit location"
		onclick={activate}
		onkeydown={activate}
	>
		<button
			type="button"
			class="flex h-4.5 w-4.5 shrink-0 cursor-pointer items-center justify-center border-none bg-transparent text-text-secondary hover:text-text-primary"
			onclick={(event) => {
				event.stopPropagation();
				onNavigate?.('');
			}}
			title="Go to root"
		>
			<Home size={14} />
		</button>
		{#if pageTitle}
			<span class="text-[13px] whitespace-nowrap text-text-primary">{pageTitle}</span>
		{:else if pathSegments.length === 0}
			<span class="text-[13px] whitespace-nowrap text-text-primary">{rootLabel()}</span>
		{:else}
			<div class="flex flex-1 items-center gap-1 overflow-hidden">
				{#each breadcrumbs as segment, index (segment.path)}
					{#if index > 0}<span class="text-xs text-text-muted">/</span>{/if}
					{#if index === breadcrumbs.length - 1}
						<span class="text-[13px] whitespace-nowrap text-text-primary">{segment.label}</span>
					{:else}
						<button
							type="button"
							class="cursor-pointer border-none bg-transparent p-0 text-[13px] text-text-secondary hover:text-white hover:underline"
							onclick={(event) => {
								event.stopPropagation();
								onNavigate?.(segment.path);
							}}>{segment.label}</button
						>
					{/if}
				{/each}
			</div>
		{/if}
	</div>
{/if}
