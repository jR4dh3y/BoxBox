<script lang="ts">
	/**
	 * Toolbar component - navigation buttons and path bar
	 */
	import {
		ChevronLeft,
		ChevronRight,
		ChevronUp,
		Home,
		RefreshCw,
		Settings,
		FolderUp,
		Folder,
		Loader2
	} from 'lucide-svelte';
	import { onDestroy, tick } from 'svelte';
	import { listDirectory, listRoots } from '$lib/api/files';
	import SearchBar from '$lib/components/SearchBar.svelte';

	interface PathSuggestion {
		path: string;
	}

	interface PathDisplayPart {
		text: string;
		selected: boolean;
		caret?: boolean;
	}

	interface Props {
		pathSegments?: string[];
		canGoBack?: boolean;
		canGoForward?: boolean;
		canGoUp?: boolean;
		onBack?: () => void;
		onForward?: () => void;
		onUp?: () => void;
		onNavigate?: (path: string) => void;
		onRefresh?: () => void;
		onSettings?: () => void;
		onUpload?: () => void;
		uploadDisabled?: boolean;
		showSearch?: boolean;
		searchValue?: string;
		searchLoading?: boolean;
		onSearchInput?: (query: string) => void;
		onSearchClear?: () => void;
		includeHiddenSuggestions?: boolean;
	}

	let {
		pathSegments = [],
		canGoBack = false,
		canGoForward = false,
		canGoUp = false,
		onBack,
		onForward,
		onUp,
		onNavigate,
		onRefresh,
		onSettings,
		onUpload,
		uploadDisabled = false,
		showSearch = false,
		searchValue = '',
		searchLoading = false,
		onSearchInput,
		onSearchClear,
		includeHiddenSuggestions = false
	}: Props = $props();

	const suggestionPageSize = 1000;
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
		return value
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
		let page = 1;
		let loadedCount = 0;
		let totalCount = Number.POSITIVE_INFINITY;
		const directories: Array<{ name: string }> = [];

		while (loadedCount < totalCount) {
			const response = await listDirectory(parentPath, {
				page,
				pageSize: suggestionPageSize,
				sortBy: 'name',
				sortDir: 'asc',
				includeHidden: includeHiddenSuggestions || prefix.startsWith('.'),
				filter: prefix || undefined
			});

			directories.push(...response.items.filter((item) => item.isDir));
			loadedCount += response.items.length;
			totalCount = response.totalCount;

			if (response.items.length === 0) break;
			page += 1;
		}

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

	const navBtnClass =
		'w-7 h-7 flex items-center justify-center bg-transparent border-none rounded text-text-secondary cursor-pointer transition-all duration-100 hover:enabled:bg-surface-elevated hover:enabled:text-text-primary disabled:text-text-disabled disabled:cursor-not-allowed';
</script>

<div
	class="relative z-50 flex items-center gap-2 border-b border-border-secondary bg-surface-primary px-3 py-1.5"
>
	<!-- Navigation buttons -->
	<div class="flex gap-0.5">
		<button type="button" class={navBtnClass} disabled={!canGoBack} onclick={onBack} title="Back">
			<ChevronLeft size={18} />
		</button>
		<button
			type="button"
			class={navBtnClass}
			disabled={!canGoForward}
			onclick={onForward}
			title="Forward"
		>
			<ChevronRight size={18} />
		</button>
		<button type="button" class={navBtnClass} disabled={!canGoUp} onclick={onUp} title="Up">
			<ChevronUp size={18} />
		</button>
	</div>

	<!-- Path bar -->
	<div
		class="relative flex min-w-0 flex-1 cursor-text items-center gap-1.5 rounded border border-border-primary bg-surface-secondary px-2 py-1 focus-within:border-border-focus"
		role="button"
		tabindex="0"
		aria-label="Edit location"
		onclick={handlePathBarClick}
		onkeydown={handlePathBarKeydown}
	>
		<button
			type="button"
			class="flex h-4.5 w-4.5 shrink-0 cursor-pointer items-center justify-center border-none bg-transparent text-text-secondary hover:text-text-primary"
			onclick={(event) => {
				event.stopPropagation();
				handleRootClick();
			}}
			title="Go to root"
		>
			<Home size={14} />
		</button>
		<div class="relative flex min-w-0 flex-1 items-center">
			{#if isEditingPath}
				<form class="relative h-5 w-full" onsubmit={handlePathSubmit}>
					<div
						class="pointer-events-none absolute inset-0 flex min-w-0 items-center overflow-hidden text-[13px] whitespace-nowrap"
						aria-hidden="true"
					>
						{#if pathDraft.length === 0}
							<span class="text-text-muted">Type a location</span>
						{:else}
							<div class="flex min-w-0 items-center overflow-hidden">
								{#each draftDisplayParts as part, index (`path-part-${index}-${part.text}-${part.selected}-${part.caret}`)}
									{#if part.caret}
										<span class="inline-block h-3.5 w-px shrink-0 animate-pulse bg-accent"></span>
									{:else}
										<span
											class="min-w-0 truncate rounded-sm {part.selected
												? 'bg-accent px-0.5 text-white'
												: 'text-text-primary'}">{part.text}</span
										>
									{/if}
								{/each}
								{#if completionDisplayText}
									<span class="min-w-0 truncate text-text-muted">{completionDisplayText}</span>
								{/if}
							</div>
						{/if}
					</div>

					<!-- Keep a real input for keyboard editing while rendering the path as custom breadcrumbs. -->
					<input
						bind:this={pathInputEl}
						type="text"
						value={pathDraft}
						oninput={handlePathInput}
						onkeydown={handlePathKeydown}
						onkeyup={syncPathSelection}
						onclick={syncPathSelection}
						onselect={syncPathSelection}
						onpointerup={syncPathSelection}
						onfocus={handlePathInputFocus}
						onblur={handlePathInputBlur}
						class="absolute inset-0 z-10 h-full w-full cursor-text border-none bg-transparent p-0 text-[13px] text-transparent caret-transparent outline-none selection:bg-transparent"
						placeholder="Type a location"
						autocomplete="off"
						autocapitalize="off"
						spellcheck="false"
						aria-label="Location"
						role="combobox"
						aria-autocomplete="list"
						aria-expanded={showSuggestionPopup}
						aria-controls={showSuggestionPopup ? suggestionListId : undefined}
						aria-activedescendant={activeSuggestionOptionId}
					/>
				</form>
			{:else if pathSegments.length === 0}
				<span class="text-[13px] whitespace-nowrap text-text-primary">This Server</span>
			{:else}
				<div class="flex flex-1 items-center gap-1 overflow-hidden">
					{#each pathSegments as segment, index (index)}
						{#if index > 0}
							<span class="text-xs text-text-muted">/</span>
						{/if}
						{#if index === pathSegments.length - 1}
							<span class="text-[13px] whitespace-nowrap text-text-primary">{segment}</span>
						{:else}
							<button
								type="button"
								class="cursor-pointer border-none bg-transparent p-0 text-[13px] text-text-secondary hover:text-white hover:underline"
								onclick={(event) => {
									event.stopPropagation();
									handleSegmentClick(index);
								}}
							>
								{segment}
							</button>
						{/if}
					{/each}
				</div>
			{/if}

			{#if showSuggestionPopup}
				<div
					id={suggestionListId}
					bind:this={suggestionListEl}
					role="listbox"
					aria-label="Location suggestions"
					class="absolute top-full right-0 left-0 z-[200] mt-1 max-h-64 overflow-auto rounded border border-border-primary bg-[#202020] py-1 shadow-2xl ring-1 ring-black/40"
				>
					{#if isLoadingSuggestions && suggestions.length === 0}
						<div class="flex items-center gap-2 px-3 py-2 text-xs text-text-secondary">
							<Loader2 size={14} class="animate-spin" />
							<span>Finding folders...</span>
						</div>
					{/if}

					{#each suggestions as suggestion, index (suggestion.path)}
						<button
							type="button"
							id={`${suggestionListId}-${index}`}
							data-suggestion-index={index}
							role="option"
							aria-selected={index === activeSuggestionIndex}
							class="flex w-full items-center gap-2 border-none px-3 py-1.5 text-left text-[13px] {index ===
							activeSuggestionIndex
								? 'bg-accent-muted text-white'
								: 'bg-transparent text-text-primary hover:bg-[#2b2b2b]'}"
							onpointerdown={(event) => handleSuggestionPointerDown(event, suggestion)}
							onmouseenter={() => {
								activeSuggestionIndex = index;
							}}
						>
							<Folder size={14} class="shrink-0 text-folder" />
							<span class="min-w-0 flex-1 truncate">{suggestion.path}</span>
						</button>
					{/each}
				</div>
			{/if}
		</div>
	</div>

	{#if showSearch}
		<div class="w-64 shrink-0 lg:w-96">
			<SearchBar
				value={searchValue}
				onInput={onSearchInput}
				onClear={onSearchClear}
				isLoading={searchLoading}
				placeholder="Search files and folders..."
				compact
			/>
		</div>
	{/if}

	<!-- Action buttons -->
	<div class="flex gap-1">
		<button
			type="button"
			class={navBtnClass}
			disabled={uploadDisabled}
			onclick={onUpload}
			title="Upload files"
		>
			<FolderUp size={16} />
		</button>
		<button type="button" class={navBtnClass} onclick={onRefresh} title="Refresh">
			<RefreshCw size={16} />
		</button>
		<button type="button" class={navBtnClass} onclick={onSettings} title="Settings">
			<Settings size={16} />
		</button>
	</div>
</div>
