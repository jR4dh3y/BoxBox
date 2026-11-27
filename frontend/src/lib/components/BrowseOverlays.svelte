<script lang="ts">
	import type { BrowseActions } from '#lib/browse/actions.svelte.js';
	import type { BrowseData } from '#lib/browse/data.svelte.js';
	import type { useFileActions } from '#lib/browse/fileActions.svelte.js';
	import type { DropQuestion } from '#lib/browse/fileDrop.js';
	import { specialEmblem, type FolderAppearance } from '#lib/browse/folderLooks.js';
	import type { HoverCards } from '#lib/browse/hoverCards.svelte.js';
	import type { BrowseLocation, HistoryPlace } from '#lib/browse/location.svelte.js';
	import type { Place, PlaceAction } from '#lib/browse/placeMenu.js';
	import type { BrowseSelection } from '#lib/browse/selection.svelte.js';
	import LoadError from './LoadError.svelte';

	let {
		actions,
		files,
		appearance,
		data,
		focused,
		hasOpenDialog,
		closeDialogs,
		hover,
		location,
		locationLabel,
		places,
		selection,
		placeLabel,
		dropQuestion,
		placeMenu,
		historyMenu,
		sidebarOpen,
		runPlaceAction,
		onCloseDrop,
		onClosePlace,
		onCloseHistory,
		shortcutsOpen,
		onCloseShortcuts,
		patternOpen,
		lastPattern,
		rememberPattern,
		onClosePattern,
		settingsOpen,
		settingsSection,
		onCloseSettings
	}: {
		actions: BrowseActions | null;
		files: ReturnType<typeof useFileActions>;
		appearance: (path: string) => FolderAppearance;
		data: BrowseData;
		focused: boolean;
		hasOpenDialog: boolean;
		closeDialogs: () => void;
		hover: HoverCards;
		location: BrowseLocation;
		locationLabel: (path: string) => string;
		places: ReadonlySet<string>;
		selection: BrowseSelection;
		placeLabel: (path: string) => string;
		dropQuestion: DropQuestion | null;
		placeMenu: { x: number; y: number; place: Place } | null;
		historyMenu: { x: number; y: number; places: HistoryPlace[] } | null;
		sidebarOpen: boolean;
		runPlaceAction: (place: Place, action: PlaceAction) => Promise<void>;
		onCloseDrop: () => void;
		onClosePlace: () => void;
		onCloseHistory: () => void;
		shortcutsOpen: boolean;
		onCloseShortcuts: () => void;
		patternOpen: boolean;
		lastPattern: string;
		rememberPattern: (pattern: string) => void;
		onClosePattern: () => void;
		settingsOpen: boolean;
		settingsSection: 'general' | 'connections';
		onCloseSettings: () => void;
	} = $props();

	function closeMenus() {
		onCloseDrop();
		onClosePlace();
		onCloseHistory();
	}
</script>

{#if actions?.previewFile && focused}
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

{#if actions && hasOpenDialog}
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

{#if files.paste.plan}
	{#await import('#lib/components/desktop/TransferConflictDialog.svelte') then { default: TransferConflictDialog }}
		<TransferConflictDialog
			plan={files.paste.plan}
			open
			onapply={files.paste.apply}
			oncancelbatch={() => void files.paste.cancel()}
		/>
	{/await}
{/if}

{#if actions?.openWithFile}
	{#await import('./desktop/OpenWithDialog.svelte') then { default: OpenWithDialog }}<OpenWithDialog
			open
			path={actions.openWithFile.path}
			onclose={actions.closeOpenWith}
		/>{/await}
{/if}

{#if actions?.propertiesItems}
	{@const items = actions.propertiesItems}
	{#await import('#lib/components/PropertiesDialog.svelte') then { default: PropertiesDialog }}
		<PropertiesDialog {items} {locationLabel} onclose={actions.closeProperties} />
	{:catch}
		<LoadError label="Properties" onClose={actions.closeProperties} />
	{/await}
{/if}

{#if dropQuestion || placeMenu || historyMenu}
	{#await import('./BrowseMenus.svelte') then { default: BrowseMenus }}
		<BrowseMenus
			{dropQuestion}
			{placeMenu}
			{historyMenu}
			aboveSidebar={sidebarOpen}
			canPaste={files.canPaste}
			{placeLabel}
			onTransfer={(action, items, destination) =>
				void files.paste.transfer(action, items, destination)}
			onPlaceAction={(place, action) => void runPlaceAction(place, action)}
			onTravel={location.travelTo}
			{onCloseDrop}
			{onClosePlace}
			{onCloseHistory}
		/>
	{:catch}
		<LoadError label="menu" onClose={closeMenus} />
	{/await}
{/if}

{#if shortcutsOpen}
	{#await import('#lib/components/KeyboardShortcutsDialog.svelte') then { default: KeyboardShortcutsDialog }}
		<KeyboardShortcutsDialog onclose={onCloseShortcuts} />
	{:catch}
		<LoadError label="keyboard shortcuts" onClose={onCloseShortcuts} />
	{/await}
{/if}

{#if patternOpen}
	{#await Promise.all( [import('#lib/components/SelectPatternDialog.svelte'), import('#lib/browse/selectPattern.js')] ) then [{ default: SelectPatternDialog }, { pathsMatching }]}
		<SelectPatternDialog
			initial={lastPattern}
			onselect={(pattern) => {
				const matched = pathsMatching(data.displayItems, pattern);
				if (matched.size > 0) {
					rememberPattern(pattern);
					selection.set(matched);
				}
				return matched.size;
			}}
			onclose={onClosePattern}
		/>
	{:catch}
		<LoadError label="Select by Pattern" onClose={onClosePattern} />
	{/await}
{/if}

{#if hover.cards?.drive.state.kind === 'shown'}
	{@const card = hover.cards.drive.state}
	{#await import('#lib/components/DriveHoverCard.svelte') then { default: DriveHoverCard }}
		<DriveHoverCard {card} />
	{/await}
{/if}

{#if hover.cards?.file.state.kind === 'shown'}
	{@const card = hover.cards.file.state}
	{#await import('#lib/components/FileHoverCard.svelte') then { default: FileHoverCard }}
		<FileHoverCard {card} {appearance} />
	{/await}
{/if}

{#if actions?.customizeFolder}
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
		<SettingsModal initialSection={settingsSection} onclose={onCloseSettings} />
	{:catch}
		<LoadError label="settings" onClose={onCloseSettings} />
	{/await}
{/if}

{#if actions?.shareItem}
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
