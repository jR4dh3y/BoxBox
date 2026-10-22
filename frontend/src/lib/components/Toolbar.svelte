<script lang="ts">
	/**
	 * Toolbar component - navigation buttons, path bar, search, and actions
	 */
	import { ChevronLeft, ChevronRight, PanelLeft, Plus, RefreshCw, Settings } from 'lucide-svelte';
	import EditablePathBar from '#lib/components/EditablePathBar.svelte';
	import SearchBar from '#lib/components/SearchBar.svelte';
	import LoadError from '#lib/components/LoadError.svelte';
	import type { AddAction } from '#lib/utils/addMenu.js';

	interface Props {
		pathSegments?: string[];
		canGoBack?: boolean;
		canGoForward?: boolean;
		onBack?: () => void;
		onForward?: () => void;
		onNavigate?: (path: string) => void;
		onRefresh?: () => void;
		onSettings?: () => void;
		/** Shows the sidebar on windows too narrow to keep it open. */
		onToggleSidebar?: () => void;
		onAdd?: (action: AddAction) => void;
		canCreate?: boolean;
		uploadDisabled?: boolean;
		showSearch?: boolean;
		searchValue?: string;
		searchLoading?: boolean;
		onSearchInput?: (query: string) => void;
		onSearchClear?: () => void;
		includeHiddenSuggestions?: boolean;
		detailsOpen?: boolean;
		onToggleDetails?: () => void;
		onShortcuts?: () => void;
	}

	let {
		pathSegments = [],
		canGoBack = false,
		canGoForward = false,
		onBack,
		onForward,
		onNavigate,
		onRefresh,
		onSettings,
		onToggleSidebar,
		onAdd,
		canCreate = false,
		uploadDisabled = false,
		showSearch = false,
		searchValue = '',
		searchLoading = false,
		onSearchInput,
		onSearchClear,
		includeHiddenSuggestions = false,
		detailsOpen = false,
		onToggleDetails,
		onShortcuts
	}: Props = $props();

	let addMenu = $state<{ x: number; y: number } | null>(null);
	const addDisabled = $derived(!canCreate && uploadDisabled);
	function openAddMenu(event: MouseEvent & { currentTarget: HTMLButtonElement }) {
		const bounds = event.currentTarget.getBoundingClientRect();
		addMenu = { x: bounds.left, y: bounds.bottom + 4 };
	}

	const navBtnClass =
		'w-7 h-7 shrink-0 flex items-center justify-center bg-transparent border-none rounded text-text-secondary cursor-pointer transition-all duration-100 hover:enabled:bg-surface-elevated hover:enabled:text-text-primary disabled:text-text-disabled disabled:cursor-not-allowed';
</script>

<div
	class="browse-chrome relative z-50 flex items-center gap-2 border-b border-border-secondary bg-surface-primary px-3 py-1.5"
>
	<!-- Navigation buttons -->
	<div class="flex shrink-0 gap-0.5">
		<button
			type="button"
			class="{navBtnClass} md:hidden"
			onclick={onToggleSidebar}
			title="Sidebar"
			aria-label="Show sidebar"
		>
			<PanelLeft size={16} />
		</button>
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
	</div>

	<EditablePathBar bind:this={pathBar} {pathSegments} {onNavigate} {includeHiddenSuggestions} />

	{#if showSearch}
		<div class="w-48 min-w-28 shrink md:w-64 lg:w-96">
			<SearchBar
				bind:this={searchBar}
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
	<div class="flex shrink-0 gap-1">
		<button
			type="button"
			class={navBtnClass}
			disabled={addDisabled}
			onclick={openAddMenu}
			title={addDisabled ? 'Nothing can be added in this location' : 'New or upload'}
			aria-haspopup="menu"
			aria-expanded={addMenu !== null}
		>
			<Plus size={16} />
		</button>
		<button type="button" class={navBtnClass} onclick={onRefresh} title="Refresh">
			<RefreshCw size={16} />
		</button>
		<button type="button" class={navBtnClass} onclick={onSettings} title="Settings">
			<Settings size={16} />
		</button>
		{#if onShortcuts}
			<button
				type="button"
				class={navBtnClass}
				onclick={onShortcuts}
				title="Keyboard shortcuts (F1)"
				aria-label="Keyboard shortcuts"><span aria-hidden="true">?</span></button
			>
		{/if}
		{#if onToggleDetails}
			<button
				bind:this={detailsToggle}
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

{#if addMenu}
	{#await import('./AddMenu.svelte') then { default: AddMenu }}
		<AddMenu
			x={addMenu.x}
			y={addMenu.y}
			{canCreate}
			canUpload={!uploadDisabled}
			onSelect={(action) => onAdd?.(action)}
			onClose={() => (addMenu = null)}
		/>
	{:catch}
		<LoadError label="add menu" onClose={() => (addMenu = null)} />
	{/await}
{/if}
