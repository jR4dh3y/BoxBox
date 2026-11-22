<script lang="ts">
	/**
	 * Browse page - the file browser. The logic lives in `#lib/browse`; this page wires it to the UI.
	 */
	import { onMount, untrack, type Snippet } from 'svelte';
	import { createDropZone } from '#lib/browse/dropZone.svelte.js';
	import { useFileDrop, type DropQuestion } from '#lib/browse/fileDrop.js';
	import { browseRequests } from '#lib/browse/requests.svelte.js';
	import { watchDesktopDirectory } from '#lib/browse/desktopWatch.js';
	import { isNativePath } from '#lib/api/desktop.js';
	import type { BrowsePaneState } from '#lib/browse/pane.svelte.js';
	import type { RemoteCatalogSnapshot } from '#lib/browse/catalog.js';
	import type { BrowseRuntime } from '#lib/browse/runtime.js';
	import Sidebar from '#lib/components/Sidebar.svelte';
	import Toolbar from '#lib/components/Toolbar.svelte';
	import FileList from '#lib/components/FileList.svelte';
	import StatusBar from '#lib/components/StatusBar.svelte';
	import LoadError from '#lib/components/LoadError.svelte';
	import Notifications from '#lib/components/Notifications.svelte';
	import { Button } from '#lib/components/ui/index.js';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { uploadStore } from '#lib/stores/upload.svelte.js';
	import { jobsStore } from '#lib/stores/jobs.svelte.js';
	import { useBrowseActions } from '#lib/browse/actions.svelte.js';
	import { useBrowseData } from '#lib/browse/data.svelte.js';
	import { useBrowseLocation, type HistoryPlace } from '#lib/browse/location.svelte.js';
	import {
		isPlaceAction,
		placeMenuItems,
		type Place,
		type PlaceAction
	} from '#lib/browse/placeMenu.js';
	import { openInNewTab } from '#lib/browse/newTab.js';
	import { toastStore } from '#lib/stores/toast.svelte.js';
	import { selectedFileBytes, useBrowseSelection } from '#lib/browse/selection.svelte.js';
	import { useBrowseUploads } from '#lib/browse/uploads.svelte.js';
	import { CONFIG } from '#lib/config.js';
	import { createHoverIntent } from '#lib/utils/hoverIntent.js';
	import type { AddAction } from '#lib/utils/addMenu.js';
	import { folderAppearance, specialEmblem } from '#lib/browse/folderLooks.js';
	import { HoverCard } from '#lib/browse/hoverCard.svelte.js';
	import { FolderPeeks } from '#lib/browse/folderPeek.svelte.js';
	import { peekDirectory, type FileInfo } from '#lib/api/files.js';
	import { parentFolder } from '#lib/browse/details.js';
	import { getFileShortcut, isTextEntryTarget } from '#lib/browse/fileShortcuts.js';
	import { invertedPaths, pathsMatching } from '#lib/browse/selectPattern.js';
	import { detailsContent } from '#lib/browse/details.js';
	import { WheelZoom } from '#lib/browse/zoom.js';

	let {
		pane,
		catalog,
		onRuntime,
		onFocus,
		onToggleSidebar,
		focused = true
	}: {
		pane?: BrowsePaneState;
		catalog?: RemoteCatalogSnapshot;
		onRuntime?: (runtime: BrowseRuntime | null) => void;
		onFocus?: () => void;
		onToggleSidebar?: () => void;
		focused?: boolean;
	} = $props();
	const paneState = untrack(() => pane);
	const selection = paneState?.selection ?? useBrowseSelection();
	const location =
		paneState?.location ??
		useBrowseLocation({
			onLocationChange: () => selection.clear(),
			selection: () => selection.paths,
			onRestoreSelection: (paths) => selection.set(new Set(paths))
		});
	const data = useBrowseData(location, selection, () => catalog);
	onMount(() => {
		onRuntime?.({ location, data });
		return () => onRuntime?.(null);
	});
	const actions = useBrowseActions(location, data, selection);
	const uploads = useBrowseUploads(location, data);
	let dropQuestion = $state<DropQuestion | null>(null);
	const dropHandler = useFileDrop(data, actions, (question) => (dropQuestion = question));
	// A drag held on a folder, a crumb or a place opens it, so a drop can go deeper.
	const drop = createDropZone(
		() => dropHandler,
		(destination) => {
			if (destination !== location.path) location.navigate(destination);
		}
	);
	const settings = $derived(settingsStore.current);
	let settingsOpen = $state(false);
	let settingsSection = $state<'general' | 'connections'>('general');
	let sidebarOpen = $state(false);
	let toolbar: Toolbar | undefined;
	function handleAdd(action: AddAction) {
		if (action === 'upload-files') uploads.openPicker();
		else if (action === 'upload-folder') uploads.openFolderPicker();
		else void actions.handleContextMenuAction(action, []);
	}
	// Places are the special folders, such as Downloads, whose folders carry an emblem.
	const places = $derived(
		new Set(data.roots.filter((root) => root.kind === 'place').map((root) => root.name))
	);
	const appearance = (path: string) =>
		folderAppearance(path, places, settingsStore.current.folderLooks);
	const hoverCard = new HoverCard({
		enabled: () => settingsStore.current.hoverDetails,
		location: (item) => (location.isSearchActive ? parentFolder(item.path) : null),
		async listFolder(path) {
			await browseRequests.prefetch(path);
			const listing = browseRequests.folder(path).value;
			if (!listing) throw new Error('The folder could not be read');
			return listing.items;
		}
	});
	// A press, scroll, key, drag or new folder covers or moves what the card describes.
	$effect(() => {
		void location.path;
		if (hasOpenDialog) hoverCard.hide();
		return () => hoverCard.hide();
	});
	const hideHoverCard = () => hoverCard.hide();
	/** The item being renamed in place, and what its field reports back. */
	const renameEditor = $derived(
		actions.renaming.kind === 'renaming'
			? {
					path: actions.renaming.file.path,
					isDir: actions.renaming.file.isDir,
					name: actions.renaming.name,
					problem: actions.renaming.problem,
					oninput: actions.setRenameName,
					oncommit: () => void actions.confirmRename(),
					oncancel: actions.cancelRename
				}
			: null
	);
	const folderPeeks = new FolderPeeks((path, hidden) => peekDirectory(path, hidden));
	const peeks = {
		get: (folder: FileInfo) => folderPeeks.get(folder, settingsStore.current.showHiddenFiles),
		request: (folder: FileInfo) =>
			folderPeeks.request(folder, settingsStore.current.showHiddenFiles)
	};
	const selectedBytes = $derived(selectedFileBytes(data.displayItems, selection.paths));
	$effect(() => {
		const path = location.path;
		if (paneState && path && isNativePath(path))
			return watchDesktopDirectory(path, () => {
				data.reload();
			});
	});
	// Uploads keep running without their panel, so closing the error only hides the message.
	let uploadErrorDismissed = $state(false);

	function closeDialogs() {
		actions.closeCreate();
		actions.closeDelete();
		actions.closeProperties();
	}
	// Overlays load on first use, so they stay out of the browse page's first download.
	const hasOpenDialog = $derived(
		actions.createDialog.open || actions.deleteDialog.open || actions.propertiesItems !== null
	);
	const onFolderHover = createHoverIntent(
		data.prefetchDirectory,
		CONFIG.query.prefetchHoverDelayMs
	);
	// A removed row never fires pointerleave, so drop a pending prefetch on navigation and teardown.
	$effect(() => {
		void location.path;
		return () => onFolderHover(null);
	});

	let shortcutsOpen = $state(false);
	function toggleDetails() {
		settingsStore.set({
			...settingsStore.current,
			detailsPanel: !settingsStore.current.detailsPanel
		});
	}
	/** A path as the path bar names it: the drive's label, then the folders under it. */
	function locationLabel(path: string): string {
		if (!path) return data.serverName ?? 'This Server';
		const [drive = '', ...rest] = path.split('/');
		const root = data.roots.find((candidate) => candidate.name === drive);
		return [root?.label ?? drive, ...rest].join('/');
	}
	const selectedItems = $derived(
		selection.count === 0 ? [] : data.displayItems.filter((item) => selection.paths.has(item.path))
	);
	let placeMenu = $state<{ x: number; y: number; place: Place } | null>(null);
	let renamingDrive = $state<string | null>(null);
	function openPlaceMenu(place: Place, x: number, y: number) {
		placeMenu = { x, y, place };
	}
	async function runPlaceAction(place: Place, action: PlaceAction) {
		switch (action) {
			case 'new-tab':
				openInNewTab(place.path);
				break;
			case 'paste':
				await actions.paste.paste(place.path);
				break;
			case 'copy-path':
				try {
					await navigator.clipboard.writeText(place.path);
				} catch {
					toastStore.error('The path could not be copied');
				}
				break;
			case 'rename-drive':
				renamingDrive = place.path;
				break;
			case 'unpin':
				settingsStore.unpinFavoriteFolder(place.path);
				break;
		}
	}
	function renameDrive(path: string, name: string | null) {
		renamingDrive = null;
		const current = settingsStore.current.driveNameOverrides[path];
		if (name === null) {
			if (current) void settingsStore.removeDriveName(path);
		} else if (
			name !== (current ?? '') &&
			name !== data.roots.find((root) => root.name === path)?.label &&
			name !== path
		) {
			void settingsStore.setDriveName(path, name);
		}
	}
	let historyMenu = $state<{ x: number; y: number; places: HistoryPlace[] } | null>(null);
	async function openHistoryMenu(back: boolean, x: number, y: number) {
		const places = await location.historyPlaces(back);
		historyMenu = places.length > 0 ? { x, y, places } : null;
	}
	/** A place's name in the history menu: the drive's label, or the folder's name. */
	function placeLabel(path: string): string {
		if (!path) return data.serverName ?? 'This Server';
		const root = data.roots.find((candidate) => candidate.name === path);
		return root?.label ?? path.split('/').at(-1) ?? path;
	}
	const wheelZoom = new WheelZoom();
	/** Ctrl+wheel over the listing zooms it instead of the page. */
	function zoomWheel(node: HTMLElement) {
		const wheel = (event: WheelEvent) => {
			if (!(event.ctrlKey || event.metaKey) || data.isAtRoot) return;
			event.preventDefault();
			const steps = wheelZoom.steps(event.deltaY, event.deltaMode);
			for (let step = 0; step < Math.abs(steps); step++)
				settingsStore.zoom(location.viewMode, steps > 0 ? 'in' : 'out');
		};
		// Not passive, so the browser's own page zoom can be held back.
		node.addEventListener('wheel', wheel, { passive: false });
		return { destroy: () => node.removeEventListener('wheel', wheel) };
	}
	let patternOpen = $state(false);
	// The last pattern comes back, as the desktop app keeps it for the session.
	let lastPattern = $state('');

	function keydown(event: KeyboardEvent) {
		if (
			!focused ||
			event.defaultPrevented ||
			document.querySelector('[role="dialog"], dialog[open], [role="menu"]')
		)
			return;
		const shortcut = getFileShortcut(event);
		if (shortcut === null) return;
		const editable = isTextEntryTarget(event.target);
		// Fields keep their own Ctrl+A, Ctrl+V and Escape; the location and search keys work anywhere.
		if (
			editable &&
			shortcut !== 'focus-location' &&
			shortcut !== 'search' &&
			shortcut !== 'shortcuts'
		)
			return;
		const inFolder = !data.isAtRoot;
		switch (shortcut) {
			case 'focus-location':
				toolbar?.focusLocation();
				break;
			case 'search':
				if (!inFolder) return;
				toolbar?.focusSearch();
				break;
			case 'shortcuts':
				shortcutsOpen = true;
				break;
			case 'details':
				toggleDetails();
				break;
			case 'rename':
				if (selectedItems.length !== 1 || data.isReadOnly) return;
				void actions.handleContextMenuAction('rename', selectedItems);
				break;
			case 'properties':
				if (selectedItems.length === 0) return;
				void actions.handleContextMenuAction('properties', selectedItems);
				break;
			case 'paste':
				if (!data.canCreate) return;
				void actions.paste.paste();
				break;
			case 'new-folder':
				if (!data.canCreate) return;
				void actions.handleContextMenuAction('new-folder', []);
				break;
			case 'select-all':
				if (!inFolder) return;
				selection.set(new Set(data.displayItems.map((item) => item.path)));
				break;
			case 'clear-selection':
				if (selection.count === 0) return;
				selection.clear();
				break;
			case 'select-pattern':
				if (!inFolder) return;
				patternOpen = true;
				break;
			case 'zoom-in':
			case 'zoom-out':
			case 'zoom-reset':
				if (!inFolder) return;
				settingsStore.zoom(
					location.viewMode,
					shortcut === 'zoom-in' ? 'in' : shortcut === 'zoom-out' ? 'out' : 'reset'
				);
				break;
			case 'invert-selection':
				if (!inFolder) return;
				selection.set(invertedPaths(data.displayItems, selection.paths));
				break;
			default:
				return;
		}
		event.preventDefault();
	}
</script>

{#snippet browserToolbar(
	extraActions?: Snippet,
	refresh: () => void = data.reload,
	refreshDisabled = false,
	pageTitle: string | null = null
)}
	<Toolbar
		bind:this={toolbar}
		{pageTitle}
		pathSegments={location.segments}
		roots={data.roots}
		serverName={data.serverName}
		canGoBack={location.canGoBack}
		canGoForward={location.canGoForward}
		onBack={location.back}
		onForward={location.forward}
		onHistoryMenu={(back, x, y) => void openHistoryMenu(back, x, y)}
		dropTarget={drop}
		onPlaceMenu={openPlaceMenu}
		onNavigate={location.navigate}
		onRefresh={refresh}
		{refreshDisabled}
		actions={extraActions}
		onToggleDetails={paneState ? undefined : toggleDetails}
		detailsOpen={settings.detailsPanel}
		onSettings={() => {
			settingsSection = 'general';
			settingsOpen = true;
		}}
		onToggleSidebar={() => (onToggleSidebar ? onToggleSidebar() : (sidebarOpen = !sidebarOpen))}
		onAdd={handleAdd}
		canCreate={data.canCreate}
		uploadDisabled={data.uploadDisabled}
		showSearch={!data.isAtRoot && !pageTitle}
		searchValue={location.searchQuery}
		searchLoading={data.isSearching}
		onSearchInput={location.search}
		onSearchClear={location.clearSearch}
		includeHiddenSuggestions={settings.showHiddenFiles}
	/>
{/snippet}

<svelte:window
	onkeydown={(event) => {
		hideHoverCard();
		keydown(event);
	}}
	onpointerdowncapture={hideHoverCard}
	onwheelcapture={hideHoverCard}
	ondragstartcapture={hideHoverCard}
	ondragend={drop.release}
	ondrop={drop.release}
	oncontextmenucapture={hideHoverCard}
	onblur={hideHoverCard}
/>

<svelte:head>
	<title>BoxBox</title>
</svelte:head>

<div
	class="browse-shell flex h-full min-h-0 w-full overflow-hidden bg-surface-primary"
	onpointerdown={onFocus}
	onfocusin={onFocus}
	role="region"
	aria-label="File browser pane"
>
	<!-- Sidebar -->
	{#if !paneState}<Sidebar
			currentPath={location.path}
			roots={data.roots}
			onNavigate={location.navigate}
			{onFolderHover}
			dropTarget={drop}
			onPlaceMenu={openPlaceMenu}
			{renamingDrive}
			onRenameDrive={renameDrive}
			open={sidebarOpen}
			onClose={() => (sidebarOpen = false)}
		/>{/if}

	<!-- Main content area -->
	<div class="flex min-h-0 min-w-0 flex-1 flex-col">
		<!-- Toolbar with navigation and path bar -->
		{#if paneState?.location.page !== 'trash'}
			{@render browserToolbar()}
		{/if}

		<!-- The listing, and the Details panel beside it. -->
		<div class="flex min-h-0 flex-1">
			<!-- File list or Drive cards -->
			<div
				class="relative min-h-0 min-w-0 flex-1 overflow-hidden"
				ondragover={(event) => drop.over(event, location.path)}
				ondragleave={drop.leave}
				ondrop={(event) => drop.drop(event, location.path)}
				role="region"
				aria-label="File browser content"
				use:zoomWheel
			>
				<!-- Drag-drop overlay -->
				{#if drop.overPath === location.path && !data.isAtRoot}
					<div
						class="pointer-events-none absolute inset-0 z-20 flex items-center justify-center border-2 border-dashed border-accent bg-accent/10"
					>
						<div class="rounded-lg bg-surface-primary/90 px-6 py-4 shadow-lg backdrop-blur-sm">
							<span class="text-lg font-medium text-accent">Drop files to upload here</span>
						</div>
					</div>
				{/if}

				{#if paneState?.location.page === 'trash'}
					{#await import('./desktop/TrashPage.svelte') then { default: TrashPage }}<TrashPage
							onChanged={() => void browseRequests.changed()}
							toolbar={browserToolbar}
							selectionCheckboxes={settings.selectionCheckboxes}
						/>{/await}
				{:else if data.isDeploymentRoot && data.deploymentError}
					<div class="space-y-3 p-6 text-sm text-text-secondary">
						<p>
							{data.deploymentError}
						</p>
						<Button
							onclick={() => {
								settingsSection = 'connections';
								settingsOpen = true;
							}}>Open Connections</Button
						>
					</div>
				{:else if data.isAtRoot}
					<!-- The drives page loads alongside the drive list, so folder pages never download it. -->
					{#await import('#lib/components/DrivesView.svelte') then { default: DrivesView }}
						<DrivesView
							drives={data.drives}
							isLoading={data.isLoadingDrives}
							viewMode={location.driveViewMode}
							compact={settings.listZoom === 0}
							onOpen={(drive) => location.navigate(drive.name)}
							{onFolderHover}
						/>
					{:catch}
						<LoadError label="drives" inline />
					{/await}
				{:else if data.fileListError && data.displayItems.length === 0}
					<div role="alert" class="flex h-full items-center justify-center gap-3 p-6 text-sm">
						<span>{data.fileListError}</span>
						<button
							type="button"
							class="rounded border border-border-primary px-3 py-1 hover:bg-surface-secondary"
							onclick={data.reload}
						>
							Retry
						</button>
					</div>
				{:else if location.viewMode === 'grid'}
					{#await import('#lib/components/FileGrid.svelte') then { default: FileGrid }}
						<FileGrid
							viewKey={location.viewKey}
							dropTarget={drop}
							items={data.displayItems}
							emptyMessage={data.emptyListMessage}
							selectedPaths={selection.paths}
							isLoading={data.isFileListLoading}
							zoom={settings.gridZoom}
							cutPaths={actions.cutPaths}
							favoritePaths={actions.favoritePaths}
							canPaste={actions.canPaste}
							canCreate={data.canCreate}
							readOnly={data.isReadOnly}
							showFileExtensions={settings.showFileExtensions}
							previewOnSingleClick={settings.previewOnSingleClick}
							selectionCheckboxes={settings.selectionCheckboxes}
							folderAppearance={appearance}
							hover={settings.hoverDetails ? hoverCard : undefined}
							peeks={settings.folderPreviews ? peeks : undefined}
							{renameEditor}
							onItemClick={actions.openFile}
							{onFolderHover}
							onSelectionChange={selection.set}
							onContextMenuAction={actions.handleContextMenuAction}
						/>
					{:catch}
						<LoadError label="grid view" inline />
					{/await}
				{:else}
					<FileList
						viewKey={location.viewKey}
						dropTarget={drop}
						items={data.displayItems}
						sortBy={location.sortBy}
						sortDir={location.sortDir}
						emptyMessage={data.emptyListMessage}
						selectedPaths={selection.paths}
						isLoading={data.isFileListLoading}
						zoom={settings.listZoom}
						cutPaths={actions.cutPaths}
						favoritePaths={actions.favoritePaths}
						canPaste={actions.canPaste}
						canCreate={data.canCreate}
						readOnly={data.isReadOnly}
						showFileExtensions={settings.showFileExtensions}
						previewOnSingleClick={settings.previewOnSingleClick}
						selectionCheckboxes={settings.selectionCheckboxes}
						folderAppearance={appearance}
						hover={settings.hoverDetails ? hoverCard : undefined}
						{renameEditor}
						onItemClick={actions.openFile}
						{onFolderHover}
						onSortChange={location.setSort}
						onSelectionChange={selection.set}
						onContextMenuAction={actions.handleContextMenuAction}
					/>
				{/if}
			</div>
			{#if settings.detailsPanel && !paneState}
				<!-- It opens by its width, so the columns make room as it comes. -->
				<div class="details-slot h-full shrink-0 overflow-hidden">
					{#await import('#lib/components/DetailsPanel.svelte') then { default: DetailsPanel }}
						<DetailsPanel
							content={detailsContent(selectedItems, data.itemCount)}
							folderName={locationLabel(location.path).split('/').at(-1) ?? ''}
							{appearance}
							{locationLabel}
						/>
					{:catch}
						<LoadError label="Details" inline />
					{/await}
				</div>
			{/if}
		</div>

		<!-- Status bar -->
		<StatusBar
			itemCount={data.itemCount}
			selectedCount={selection.count}
			{selectedBytes}
			viewMode={data.isAtRoot ? location.driveViewMode : location.viewMode}
			onViewModeChange={location.setViewMode}
		/>
	</div>
</div>

{#if actions.previewFile && focused}
	{#await import('#lib/components/FilePreview.svelte') then { default: FilePreview }}
		<FilePreview
			file={actions.previewFile}
			allFiles={data.previewableFiles}
			onNavigate={actions.showPreview}
			onFileSaved={actions.fileSaved}
			onClose={actions.closePreview}
		/>
	{:catch}
		<LoadError label="preview" onClose={actions.closePreview} />
	{/await}
{/if}

{#if hasOpenDialog}
	{#await import('#lib/components/BrowseDialogs.svelte') then { default: BrowseDialogs }}
		<BrowseDialogs
			createDialog={actions.createDialog}
			deleteDialog={actions.deleteDialog}
			onCreateNameChange={actions.setCreateName}
			onCreateConfirm={() => void actions.confirmCreate()}
			onDeleteConfirm={() => void actions.confirmDelete()}
			onCloseCreate={actions.closeCreate}
			onCloseDelete={actions.closeDelete}
		/>
	{:catch}
		<LoadError label="dialog" onClose={closeDialogs} />
	{/await}
{/if}

{#if actions.paste.plan}
	{#await import('#lib/components/desktop/TransferConflictDialog.svelte') then { default: TransferConflictDialog }}
		<TransferConflictDialog
			plan={actions.paste.plan}
			open
			onapply={actions.paste.apply}
			oncancelbatch={() => void actions.paste.cancel()}
		/>
	{/await}
{/if}

{#if actions.openWithFile}
	{#await import('./desktop/OpenWithDialog.svelte') then { default: OpenWithDialog }}<OpenWithDialog
			open
			path={actions.openWithFile.path}
			onclose={actions.closeOpenWith}
		/>{/await}
{/if}

{#if actions.propertiesItems}
	{@const items = actions.propertiesItems}
	{#await import('#lib/components/PropertiesDialog.svelte') then { default: PropertiesDialog }}
		<PropertiesDialog {items} {locationLabel} onclose={actions.closeProperties} />
	{:catch}
		<LoadError label="Properties" onClose={actions.closeProperties} />
	{/await}
{/if}

{#if dropQuestion}
	{@const question = dropQuestion}
	{#await import('#lib/components/ui/ContextMenu.svelte') then { default: ContextMenu }}
		<ContextMenu
			items={[
				...(question.canMove ? [{ id: 'move', label: 'Move Here' }] : []),
				{ id: 'copy', label: 'Copy Here' },
				{ id: 'separator', label: '', separator: true },
				{ id: 'cancel', label: 'Cancel' }
			]}
			x={question.x}
			y={question.y}
			onSelect={(id) => {
				if (id === 'move' || id === 'copy')
					void actions.paste.transfer(id, question.items, question.destination);
			}}
			onClose={() => (dropQuestion = null)}
		/>
	{/await}
{/if}

{#if placeMenu}
	{@const menu = placeMenu}
	{#await import('#lib/components/ui/ContextMenu.svelte') then { default: ContextMenu }}
		<ContextMenu
			items={placeMenuItems(menu.place, actions.canPaste)}
			x={menu.x}
			y={menu.y}
			onSelect={(id) => {
				if (isPlaceAction(id)) void runPlaceAction(menu.place, id);
			}}
			onClose={() => (placeMenu = null)}
		/>
	{/await}
{/if}

{#if historyMenu}
	{@const menu = historyMenu}
	{#await import('#lib/components/ui/ContextMenu.svelte') then { default: ContextMenu }}
		<ContextMenu
			items={menu.places.map((place) => ({
				id: String(place.index),
				label: placeLabel(place.path)
			}))}
			x={menu.x}
			y={menu.y}
			onSelect={(id) => location.travelTo(Number(id))}
			onClose={() => (historyMenu = null)}
		/>
	{/await}
{/if}

{#if shortcutsOpen}
	{#await import('#lib/components/KeyboardShortcutsDialog.svelte') then { default: KeyboardShortcutsDialog }}
		<KeyboardShortcutsDialog onclose={() => (shortcutsOpen = false)} />
	{:catch}
		<LoadError label="keyboard shortcuts" onClose={() => (shortcutsOpen = false)} />
	{/await}
{/if}

{#if patternOpen}
	{#await import('#lib/components/SelectPatternDialog.svelte') then { default: SelectPatternDialog }}
		<SelectPatternDialog
			initial={lastPattern}
			onselect={(pattern) => {
				const matched = pathsMatching(data.displayItems, pattern);
				if (matched.size > 0) {
					lastPattern = pattern;
					selection.set(matched);
				}
				return matched.size;
			}}
			onclose={() => (patternOpen = false)}
		/>
	{:catch}
		<LoadError label="Select by Pattern" onClose={() => (patternOpen = false)} />
	{/await}
{/if}

{#if hoverCard.state.kind === 'shown'}
	{@const card = hoverCard.state}
	{#await import('#lib/components/FileHoverCard.svelte') then { default: FileHoverCard }}
		<FileHoverCard {card} {appearance} />
	{/await}
{/if}

{#if actions.customizeFolder}
	{@const folder = actions.customizeFolder}
	{#await import('#lib/components/FolderCustomizeDialog.svelte') then { default: FolderCustomizeDialog }}
		<FolderCustomizeDialog
			{folder}
			own={specialEmblem(folder.path, places)}
			onclose={actions.closeCustomize}
		/>
	{:catch}
		<LoadError label="Customize Folder" onClose={actions.closeCustomize} />
	{/await}
{/if}

{#if settingsOpen}
	{#await import('#lib/components/settings/SettingsModal.svelte') then { default: SettingsModal }}
		<SettingsModal initialSection={settingsSection} onclose={() => (settingsOpen = false)} />
	{:catch}
		<LoadError label="settings" onClose={() => (settingsOpen = false)} />
	{/await}
{/if}

{#if actions.shareItem}
	{#await import('#lib/components/ShareModal.svelte') then { default: ShareModal }}
		<ShareModal
			open
			item={actions.shareItem}
			readOnly={data.isReadOnly}
			onclose={actions.closeShare}
		/>
	{:catch}
		<LoadError label="share window" onClose={actions.closeShare} />
	{/await}
{/if}

<!-- Hidden file input for upload button -->
<input
	bind:this={uploads.fileInput}
	type="file"
	multiple
	class="hidden"
	onchange={uploads.handleInputChange}
/>
<input
	bind:this={uploads.folderInput}
	type="file"
	webkitdirectory
	class="hidden"
	onchange={uploads.handleInputChange}
/>

<!-- Toasts, a failed refresh and the upload panel stack in one column. -->
{#if focused}<Notifications>
		{#if jobsStore.jobs.size}{#await import('./desktop/TransferJobs.svelte') then { default: TransferJobs }}<TransferJobs
				/>{/await}{/if}
		{#if data.fileListError && data.displayItems.length > 0}
			<!-- The stale list stays visible, so the failure is a notice instead of a page error. -->
			<div
				role="alert"
				class="pointer-events-auto flex items-center gap-3 rounded-lg border border-border-primary bg-surface-secondary py-2 pr-2 pl-4 text-[13px] shadow-lg"
			>
				<span class="min-w-0 flex-1 text-text-primary">{data.fileListError}</span>
				<Button variant="secondary" size="sm" onclick={data.reload}>Retry</Button>
			</div>
		{/if}
		{#if uploadStore.uploads.length > 0}
			{#await import('#lib/components/UploadPanel.svelte') then { default: UploadPanel }}
				<UploadPanel />
			{:catch}
				{#if !uploadErrorDismissed}
					<LoadError label="upload panel" onClose={() => (uploadErrorDismissed = true)} />
				{/if}
			{/await}
		{/if}
	</Notifications>{/if}

<style>
	.details-slot {
		animation: details-open 200ms cubic-bezier(0.2, 0, 0, 1);
	}
	@keyframes details-open {
		from {
			width: 0;
			opacity: 0;
		}
		to {
			width: 260px;
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.details-slot {
			animation: none;
		}
	}
</style>
