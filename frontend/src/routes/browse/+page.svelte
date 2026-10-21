<script lang="ts">
	/** The browse route owns shared navigation chrome; each visible pane owns its file state. */
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { tick, untrack } from 'svelte';
	import Sidebar from '#lib/components/Sidebar.svelte';
	import Toolbar from '#lib/components/Toolbar.svelte';
	import BrowseTabs from '#lib/components/desktop/BrowseTabs.svelte';
	import LoadError from '#lib/components/LoadError.svelte';
	import Toast from '#lib/components/ui/Toast.svelte';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { uploadStore } from '#lib/stores/upload.svelte.js';
	import { parseBrowseLaunchState } from '#lib/browse/pane.svelte.js';
	import type { BrowsePaneRuntime } from '#lib/browse/pane.svelte.js';
	import { useBrowseTabs } from '#lib/browse/tabs.svelte.js';
	import type { BrowseLocationSnapshot } from '#lib/browse/location.svelte.js';
	import { getBrowseShortcut } from '#lib/browse/paneShortcuts.js';
	import { CONFIG } from '#lib/config.js';
	import { createHoverIntent } from '#lib/utils/hoverIntent.js';

	const browsePane = import('#lib/components/desktop/BrowsePane.svelte');
	const launchState = parseBrowseLaunchState(page.url);
	let pendingSelectPath = $state(launchState.selectPath);
	let paneRuntimes = $state.raw<Record<string, BrowsePaneRuntime>>({});
	let uploadErrorDismissed = $state(false);
	let launchTabId = $state('');

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
	const settings = $derived(settingsStore.current);

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
		if (snapshot.path) url.searchParams.set('path', snapshot.path);
		else url.searchParams.delete('path');
		if (snapshot.searchQuery) url.searchParams.set('q', snapshot.searchQuery);
		else url.searchParams.delete('q');
		url.searchParams.set('sort', snapshot.sortBy);
		url.searchParams.set('dir', snapshot.sortDir);
		url.searchParams.set('view', snapshot.viewMode);
		if (selectPath) url.searchParams.set('select', selectPath);
		else url.searchParams.delete('select');

		void goto(`${resolve('browse')}${url.search}${url.hash}`, {
			replace: true,
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

	function isEditableTarget(target: EventTarget | null): boolean {
		return (
			target instanceof HTMLElement &&
			(target.isContentEditable ||
				target.closest('input, textarea, select, [contenteditable="true"]') !== null)
		);
	}

	function focusLocation(): void {
		const input = document.querySelector<HTMLInputElement>('input[aria-label="Location"]');
		if (input) {
			input.focus();
			input.select();
			return;
		}
		const pathBar = document.querySelector<HTMLElement>('[aria-label="Edit location"]');
		pathBar?.click();
		void tick().then(() => {
			const editor = document.querySelector<HTMLInputElement>('input[aria-label="Location"]');
			editor?.focus();
			editor?.select();
		});
	}

	function handleKeydown(event: KeyboardEvent): void {
		if (event.defaultPrevented) return;
		const workspace = document.getElementById('browse-workspace');
		const insideWorkspace =
			event.target instanceof Node && workspace?.contains(event.target) === true;
		if (!insideWorkspace && !activeRuntime?.hasOpenDialog) return;

		const shortcut = getBrowseShortcut({
			key: event.key,
			ctrlKey: event.ctrlKey,
			metaKey: event.metaKey,
			shiftKey: event.shiftKey,
			altKey: event.altKey,
			editable: isEditableTarget(event.target)
		});
		if (!shortcut) return;
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
				settingsStore.set({
					...settingsStore.current,
					showHiddenFiles: !settingsStore.current.showHiddenFiles
				});
				break;
			case 'paste':
				void activeRuntime?.actions.paste.paste();
				break;
			case 'focus-location':
				focusLocation();
				break;
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<svelte:head>
	<title>BoxBox</title>
</svelte:head>

<div id="browse-workspace" class="flex h-screen w-full overflow-hidden bg-surface-primary">
	<Sidebar
		currentPath={activeLocation?.path ?? ''}
		roots={activeRuntime?.data.roots ?? []}
		onNavigate={(path) => activeLocation?.navigate(path)}
		{onFolderHover}
	/>

	<div class="flex min-w-0 flex-1 flex-col">
		<Toolbar
			pathSegments={activeLocation?.segments ?? []}
			canGoBack={activeLocation?.canGoBack ?? false}
			canGoForward={activeLocation?.canGoForward ?? false}
			canGoUp={activeLocation?.canGoUp ?? false}
			onBack={() => activeLocation?.back()}
			onForward={() => activeLocation?.forward()}
			onUp={() => activeLocation?.up()}
			onNavigate={(path) => activeLocation?.navigate(path)}
			onRefresh={() => activeRuntime?.data.reload()}
			onSettings={() => goto(resolve('settings'))}
			onUpload={() => activeRuntime?.uploads.openPicker()}
			onUploadFolder={() => activeRuntime?.uploads.openFolderPicker()}
			uploadDisabled={activeRuntime?.data.uploadDisabled ?? true}
			showSearch={activeLocation !== undefined && activeLocation.path !== ''}
			searchValue={activeLocation?.searchQuery ?? ''}
			searchLoading={activeRuntime?.data.isSearching ?? false}
			onSearchInput={(query) => activeLocation?.search(query)}
			onSearchClear={() => activeLocation?.clearSearch()}
			includeHiddenSuggestions={settings.showHiddenFiles}
		/>

		<BrowseTabs controller={tabs} onTabChange={focusActivePane} />

		<div class="flex min-h-0 flex-1 divide-x divide-border-secondary">
			{#await browsePane then { default: BrowsePane }}
				{#each tabs.visiblePanes as pane (pane.id)}
					<BrowsePane
						{pane}
						focused={pane.id === activePane?.id}
						initialSelectPath={pendingSelectPath &&
						tabs.activeTabId === launchTabId &&
						pane.id === activePane?.id
							? pendingSelectPath
							: null}
						onFocus={() => tabs.focusPane(pane.id)}
						onRuntimeChange={(runtime) => setPaneRuntime(pane.id, runtime)}
						onInitialSelectHandled={handleInitialSelectHandled}
					/>
				{/each}
			{:catch}
				<LoadError label="file browser" />
			{/await}
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
