<script lang="ts">
	/** The browse route owns shared navigation chrome; each visible pane owns its file state. */
	import { goto, afterNavigate } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { tick, untrack } from 'svelte';
	import Sidebar from '#lib/components/Sidebar.svelte';
	import LoadError from '#lib/components/LoadError.svelte';
	import Toast from '#lib/components/ui/Toast.svelte';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { uploadStore } from '#lib/stores/upload.svelte.js';
	import { parseBrowseLaunchState } from '#lib/browse/pane.svelte.js';
	import type { BrowsePaneRuntime } from '#lib/browse/pane.svelte.js';
	import { useBrowseTabs } from '#lib/browse/tabs.svelte.js';
	import { parseBrowseLocation, type BrowseLocationSnapshot } from '#lib/browse/location.svelte.js';
	import { getBrowseShortcut } from '#lib/browse/paneShortcuts.js';
	import { isTextEntryTarget } from '#lib/browse/fileShortcuts.js';
	import type ToolbarComponent from '#lib/components/Toolbar.svelte';
	import { CONFIG } from '#lib/config.js';
	import { createHoverIntent } from '#lib/utils/hoverIntent.js';
	import { isLocalDesktop, isNativePath } from '#lib/api/desktop.js';
	import { locationTitle } from '#lib/browse/locationTitle.js';
	import { createDropZone } from '#lib/browse/dropZone.svelte.js';
	import { fileDragKind } from '#lib/browse/fileDrag.js';

	// Local folders refresh themselves and are already on this computer, so there is nothing to upload.
	const desktop = isLocalDesktop();
	const browsePane = import('#lib/components/desktop/BrowsePane.svelte');
	const toolbar = import('#lib/components/Toolbar.svelte');
	const launchState = parseBrowseLaunchState(page.url);
	let pendingSelectPath = $state(launchState.selectPath);
	let paneRuntimes = $state.raw<Record<string, BrowsePaneRuntime>>({});
	let uploadErrorDismissed = $state(false);
	let launchTabId = $state('');
	let detailsOpen = $state(false);
	let shortcutsOpen = $state(false);
	let toolbarRef = $state<ReturnType<typeof ToolbarComponent>>();

	const tabs = useBrowseTabs({
		initialLocation: launchState.location,
		onActiveLocationChange(snapshot) {
			if (pendingSelectPath && tabs.activeTabId !== launchTabId) pendingSelectPath = null;
			syncBrowseUrl(snapshot, pendingSelectPath);
		}
	});
	launchTabId = tabs.activeTabId;

	const activePane = $derived(tabs.activePane);
	const activeLocation = $derived(activePane?.location);
	const activeRuntime = $derived(activePane ? (paneRuntimes[activePane.id] ?? null) : null);
	const sidebarDrop = createDropZone(() => activeRuntime?.drop);
	const settings = $derived(settingsStore.current);

	function openInNewTab(path: string): void {
		if (!activeLocation) return;
		tabs.newTab({ ...activeLocation.snapshot, page: 'files', path, searchQuery: '' });
		focusActivePane();
	}

	function closeDetails(): void {
		detailsOpen = false;
		void tick().then(() => toolbarRef?.focusDetailsToggle());
	}

	afterNavigate(({ type }) => {
		if (type === 'popstate') {
			untrack(() => activeLocation?.restore(parseBrowseLocation(page.url)));
		}
	});

	const onFolderHover = createHoverIntent((path) => {
		if (path) activeRuntime?.data.prefetchDirectory(path);
	}, CONFIG.query.prefetchHoverDelayMs);

	$effect(() => {
		void activePane?.id;
		void activeLocation?.path;
		return () => onFolderHover(null);
	});

	function syncBrowseUrl(snapshot: BrowseLocationSnapshot, selectPath: string | null): void {
		const url = new URL(page.url.href);
		if (snapshot.page !== 'files') url.searchParams.set('page', snapshot.page);
		else url.searchParams.delete('page');
		if (snapshot.path) url.searchParams.set('path', snapshot.path);
		else url.searchParams.delete('path');
		if (snapshot.searchQuery) url.searchParams.set('q', snapshot.searchQuery);
		else url.searchParams.delete('q');
		url.searchParams.set('sort', snapshot.sortBy);
		url.searchParams.set('dir', snapshot.sortDir);
		url.searchParams.set('view', snapshot.viewMode);
		if (selectPath) url.searchParams.set('select', selectPath);
		else url.searchParams.delete('select');
		if (url.href === page.url.href) return;

		void goto(`${resolve('browse')}${url.search}${url.hash}`, {
			replace:
				(page.url.searchParams.get('path') ?? '') === snapshot.path &&
				(page.url.searchParams.get('page') ?? 'files') === snapshot.page,
			reset: false,
			state: page.state
		});
	}

	function setPaneRuntime(paneId: string, runtime: BrowsePaneRuntime | null): void {
		if (runtime) {
			paneRuntimes = { ...paneRuntimes, [paneId]: runtime };
			return;
		}

		if (!(paneId in paneRuntimes)) return;
		const next = { ...paneRuntimes };
		delete next[paneId];
		paneRuntimes = next;
	}

	function handleInitialSelectHandled(path: string): void {
		if (untrack(() => pendingSelectPath) !== path) return;
		pendingSelectPath = null;
		if (activePane) syncBrowseUrl(activePane.location.snapshot, null);
	}

	function focusActivePane(): void {
		const paneId = tabs.activePane?.id;
		if (!paneId) return;
		void tick().then(() => document.getElementById(`browse-pane-${paneId}`)?.focus());
	}

	/** A file dropped outside a drop target would replace the app with the file. */
	function refuseStrayFileDrop(event: DragEvent): void {
		if (event.defaultPrevented || fileDragKind(event.dataTransfer) === null) return;
		event.preventDefault();
		if (event.dataTransfer) event.dataTransfer.dropEffect = 'none';
	}

	function handleKeydown(event: KeyboardEvent): void {
		if (event.defaultPrevented || shortcutsOpen) return;
		if (event.key === 'F1' && !activeRuntime?.hasOpenDialog) {
			event.preventDefault();
			shortcutsOpen = true;
			return;
		}
		const workspace = document.getElementById('browse-workspace');
		const insideWorkspace =
			event.target instanceof Node && workspace?.contains(event.target) === true;
		if (!insideWorkspace && event.target !== document.body && !activeRuntime?.hasOpenDialog) return;

		const shortcut = getBrowseShortcut({
			key: event.key,
			ctrlKey: event.ctrlKey,
			metaKey: event.metaKey,
			shiftKey: event.shiftKey,
			altKey: event.altKey,
			editable: isTextEntryTarget(event.target)
		});
		if (!shortcut || (shortcut === 'open-servers' && !desktop)) return;
		event.preventDefault();
		if (activeRuntime?.hasOpenDialog) return;

		switch (shortcut) {
			case 'new-tab':
				tabs.newTab();
				focusActivePane();
				break;
			case 'close-tab':
				tabs.closeTab();
				focusActivePane();
				break;
			case 'next-tab':
				tabs.cycleTab(1);
				focusActivePane();
				break;
			case 'previous-tab':
				tabs.cycleTab(-1);
				focusActivePane();
				break;
			case 'toggle-split':
				tabs.toggleSplit();
				focusActivePane();
				break;
			case 'back':
				activeLocation?.back();
				break;
			case 'forward':
				activeLocation?.forward();
				break;
			case 'up':
				activeLocation?.up();
				break;
			case 'refresh':
				activeRuntime?.data.reload();
				break;
			case 'toggle-hidden':
				settingsStore.toggleHiddenFiles();
				break;
			case 'focus-location':
				toolbarRef?.focusLocation();
				break;
			case 'open-servers':
				if (desktop) activeLocation?.openPage('servers');
				break;
		}
	}
</script>

<svelte:window
	onkeydown={handleKeydown}
	ondragover={refuseStrayFileDrop}
	ondrop={refuseStrayFileDrop}
/>

<svelte:head>
	<title>BoxBox</title>
</svelte:head>

<div id="browse-workspace" class="flex h-screen w-full overflow-hidden bg-surface-primary">
	<Sidebar
		currentPath={activeLocation?.path ?? ''}
		currentPage={activeLocation?.page ?? 'files'}
		onOpenPage={(page) => activeLocation?.openPage(page)}
		dropTarget={sidebarDrop}
		roots={activeRuntime?.data.roots ?? []}
		onNavigate={(path) => activeLocation?.navigate(path)}
		{onFolderHover}
	/>

	<div class="flex min-w-0 flex-1 flex-col">
		{#await toolbar then { default: Toolbar }}
			<Toolbar
				bind:this={toolbarRef}
				pathSegments={activeLocation?.segments ?? []}
				roots={activeRuntime?.data.roots ?? []}
				pageTitle={activeLocation && activeLocation.page !== 'files'
					? locationTitle(activeLocation)
					: null}
				canGoBack={activeLocation?.canGoBack ?? false}
				canGoForward={activeLocation?.canGoForward ?? false}
				canGoUp={activeLocation?.canGoUp ?? false}
				onBack={() => activeLocation?.back()}
				onForward={() => activeLocation?.forward()}
				onUp={() => activeLocation?.up()}
				onNavigate={(path) => activeLocation?.navigate(path)}
				onRefresh={() => activeRuntime?.data.reload()}
				onSettings={() => goto(resolve('settings'))}
				onUpload={isNativePath(activeLocation?.path ?? '')
					? undefined
					: () => activeRuntime?.uploads.openPicker()}
				onUploadFolder={isNativePath(activeLocation?.path ?? '')
					? undefined
					: () => activeRuntime?.uploads.openFolderPicker()}
				uploadDisabled={activeRuntime?.data.uploadDisabled ?? true}
				showSearch={activeLocation !== undefined && activeLocation.path !== ''}
				searchValue={activeLocation?.searchQuery ?? ''}
				searchLoading={activeRuntime?.data.isSearching ?? false}
				onSearchInput={(query) => activeLocation?.search(query)}
				onSearchClear={() => activeLocation?.clearSearch()}
				includeHiddenSuggestions={settings.showHiddenFiles}
				{detailsOpen}
				onToggleDetails={() => (detailsOpen = !detailsOpen)}
				onShortcuts={() => (shortcutsOpen = true)}
				splitOpen={tabs.hasSplit}
				onToggleSplit={() => {
					tabs.toggleSplit();
					focusActivePane();
				}}
			/>
		{:catch}
			<LoadError label="file toolbar" />
		{/await}

		{#if tabs.tabs.length > 1}
			{#await import('#lib/components/desktop/BrowseTabs.svelte') then { default: BrowseTabs }}
				<BrowseTabs controller={tabs} onTabChange={focusActivePane} />
			{:catch}
				<LoadError label="tabs" />
			{/await}
		{/if}

		<div class="relative flex min-h-0 flex-1">
			<div class="flex min-h-0 min-w-0 flex-1 divide-x divide-border-secondary">
				{#await browsePane then { default: BrowsePane }}
					{#each tabs.visiblePanes as pane (pane.id)}
						<BrowsePane
							{pane}
							focused={pane.id === activePane?.id}
							outlined={tabs.hasSplit && pane.id === activePane?.id}
							initialSelectPath={pendingSelectPath &&
							tabs.activeTabId === launchTabId &&
							pane.id === activePane?.id
								? pendingSelectPath
								: null}
							onFocus={() => tabs.focusPane(pane.id)}
							onRuntimeChange={(runtime) => setPaneRuntime(pane.id, runtime)}
							onInitialSelectHandled={handleInitialSelectHandled}
							onShowDetails={() => (detailsOpen = true)}
							onFocusSearch={() => toolbarRef?.focusSearch()}
							onOpenInNewTab={openInNewTab}
						/>
					{/each}
				{:catch}
					<LoadError label="file browser" />
				{/await}
			</div>
			{#if detailsOpen}
				<div
					class="fixed inset-y-0 right-0 z-[60] w-[min(20rem,100%)] shadow-xl lg:static lg:z-auto lg:w-72 lg:shrink-0 lg:shadow-none"
				>
					{#await import('#lib/components/FileDetailsPanel.svelte') then { default: FileDetailsPanel }}
						<FileDetailsPanel runtime={activeRuntime} onClose={closeDetails} />
					{:catch}
						<LoadError label="selection details" onClose={closeDetails} />
					{/await}
				</div>
			{/if}
		</div>
	</div>
</div>

{#if uploadStore.uploads.length > 0}
	{#await import('#lib/components/UploadPanel.svelte') then { default: UploadPanel }}
		<UploadPanel />
	{:catch}
		{#if !uploadErrorDismissed}
			<LoadError label="upload panel" onClose={() => (uploadErrorDismissed = true)} />
		{/if}
	{/await}
{/if}

<Toast />

{#if desktop}
	{#await import('#lib/components/desktop/DesktopUndoShortcut.svelte') then { default: DesktopUndoShortcut }}
		<DesktopUndoShortcut />
	{/await}
{/if}

{#if shortcutsOpen}
	{#await import('#lib/components/KeyboardShortcutsDialog.svelte') then { default: KeyboardShortcutsDialog }}
		<KeyboardShortcutsDialog onclose={() => (shortcutsOpen = false)} />
	{:catch}
		<LoadError label="keyboard shortcuts" onClose={() => (shortcutsOpen = false)} />
	{/await}
{/if}
