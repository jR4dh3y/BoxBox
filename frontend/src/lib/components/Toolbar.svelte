<script lang="ts">
	/**
	 * Toolbar component - navigation buttons, path bar, search, and actions
	 */
	import {
		ChevronLeft,
		ChevronRight,
		ChevronUp,
		FolderUp,
		RefreshCw,
		Settings,
		Upload,
		PanelRight
	} from 'lucide-svelte';
	import EditablePathBar from '#lib/components/EditablePathBar.svelte';
	import SearchBar from '#lib/components/SearchBar.svelte';
	import { isLocalDesktop } from '#lib/api/desktop.js';

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
		onUploadFolder?: () => void;
		uploadDisabled?: boolean;
		showSearch?: boolean;
		searchValue?: string;
		searchLoading?: boolean;
		onSearchInput?: (query: string) => void;
		onSearchClear?: () => void;
		includeHiddenSuggestions?: boolean;
		detailsOpen?: boolean;
		onToggleDetails?: () => void;
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
		onUploadFolder,
		uploadDisabled = false,
		showSearch = false,
		searchValue = '',
		searchLoading = false,
		onSearchInput,
		onSearchClear,
		includeHiddenSuggestions = false,
		detailsOpen = false,
		onToggleDetails
	}: Props = $props();

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

	<EditablePathBar {pathSegments} {onNavigate} {includeHiddenSuggestions} />

	{#if showSearch}
		<div class="w-40 min-w-24 shrink lg:w-64">
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
			<Upload size={16} />
		</button>
		<button
			type="button"
			class={navBtnClass}
			disabled={uploadDisabled}
			onclick={onUploadFolder}
			title="Upload folder"
		>
			<FolderUp size={16} />
		</button>
		<button type="button" class={navBtnClass} onclick={onRefresh} title="Refresh">
			<RefreshCw size={16} />
		</button>
		<button type="button" class={navBtnClass} onclick={onSettings} title="Settings">
			<Settings size={16} />
		</button>
		{#if onToggleDetails}
			<button
				type="button"
				class="inline-flex h-7 items-center gap-1.5 rounded px-2 text-xs transition-colors hover:bg-surface-elevated focus-visible:outline-2 focus-visible:outline-accent {detailsOpen
					? 'bg-selection text-text-primary'
					: 'text-text-secondary'}"
				aria-label="Details"
				aria-pressed={detailsOpen}
				onclick={onToggleDetails}
				title="Selection details"
			>
				<PanelRight size={16} /><span class="hidden xl:inline">Details</span>
			</button>
		{/if}
	</div>
</div>

{#if isLocalDesktop()}
	{#await import('#lib/components/desktop/DesktopTools.svelte') then { default: DesktopTools }}
		<DesktopTools path={pathSegments.join('/')} {onNavigate} />
	{/await}
{/if}
