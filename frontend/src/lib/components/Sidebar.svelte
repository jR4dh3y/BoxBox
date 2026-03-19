<script lang="ts">
	/**
	 * Sidebar component - navigation panel with storage, places, favorites
	 */
	import type { MountPoint } from '$lib/api/files';
	import {
		ChevronDown,
		HardDrive,
		Server,
		Monitor,
		Download,
		FileText,
		Music,
		Image,
		Video,
		Star,
		Pencil,
		X,
		MoreVertical
	} from 'lucide-svelte';
	import { Badge, ContextMenu, InlineRename } from '$lib/components/ui';
	import { settingsStore } from '$lib/stores/settings.svelte';

	interface Props {
		roots?: MountPoint[];
		currentPath?: string;
		onNavigate?: (path: string) => void;
	}

	let { roots = [], currentPath = '', onNavigate }: Props = $props();

	// Quick access places
	const places = [
		{ name: 'This Server', path: '', icon: Server },
		{ name: 'Desktop', path: 'desktop', icon: Monitor },
		{ name: 'Downloads', path: 'downloads', icon: Download },
		{ name: 'Documents', path: 'documents', icon: FileText },
		{ name: 'Music', path: 'music', icon: Music },
		{ name: 'Pictures', path: 'pictures', icon: Image },
		{ name: 'Videos', path: 'videos', icon: Video }
	];

	// Favorites (could be stored in localStorage later)
	let favorites = $state<{ name: string; path: string }[]>([]);

	function isActive(path: string): boolean {
		if (path === '' && currentPath === '') return true;
		return currentPath.startsWith(path) && path !== '';
	}

	function handleNavigate(path: string) {
		onNavigate?.(path);
	}

	// Collapsed sections state
	let storageCollapsed = $state(false);
	let placesCollapsed = $state(false);
	let favoritesCollapsed = $state(false);

	// Renaming state
	let renamingDrive = $state<string | null>(null);
	let contextMenuOpen = $state<string | null>(null);
	let contextMenuPosition = $state({ x: 0, y: 0 });
	const driveNameOverrides = $derived(settingsStore.driveNameOverrides);

	const navItemClass =
		'w-full flex items-center gap-2.5 py-1.5 px-3 pl-5 bg-transparent border-none text-text-primary text-[13px] cursor-pointer text-left transition-colors duration-100 hover:bg-surface-secondary';
	const navItemActiveClass = 'bg-selection text-white hover:bg-selection-hover';
	const storageRowClass =
		'group relative flex items-center gap-2.5 py-1.5 px-3 pl-5 text-[13px] transition-colors duration-100';
	const storageRowButtonClass =
		'flex items-center gap-2.5 flex-1 min-w-0 bg-transparent border-none p-0 text-left text-inherit cursor-pointer';

	function getDriveName(root: MountPoint): string {
		return driveNameOverrides[root.name] || root.name;
	}

	function startRenaming(rootName: string) {
		renamingDrive = rootName;
		contextMenuOpen = null;
	}

	function handleSaveRename(originalName: string, newValue: string) {
		if (newValue && newValue !== originalName) {
			settingsStore.setDriveName(originalName, newValue);
		} else if (!newValue) {
			settingsStore.removeDriveName(originalName);
		}
		renamingDrive = null;
	}

	function handleCancelRename() {
		renamingDrive = null;
	}

	function resetDriveName(originalName: string) {
		settingsStore.removeDriveName(originalName);
		contextMenuOpen = null;
	}

	function handleContextMenu(rootName: string, e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
		contextMenuPosition = { x: e.clientX, y: e.clientY };
		contextMenuOpen = rootName;
	}

	function handleMenuSelect(rootName: string, id: string) {
		if (id === 'rename') {
			startRenaming(rootName);
		} else if (id === 'reset') {
			resetDriveName(rootName);
		}
	}

	function handleMenuClose() {
		contextMenuOpen = null;
	}
</script>

<aside
	class="flex w-[220px] min-w-[220px] flex-col overflow-x-hidden overflow-y-auto border-r border-border-secondary bg-surface-primary"
>
	<!-- Storage Section -->
	<div class="border-b border-border-secondary">
		<button
			type="button"
			class="flex w-full cursor-pointer items-center gap-1.5 border-none bg-transparent px-3 py-2.5 text-left text-[11px] font-medium tracking-wide text-text-secondary uppercase hover:text-text-primary"
			onclick={() => (storageCollapsed = !storageCollapsed)}
		>
			<ChevronDown
				size={14}
				class="shrink-0 transition-transform duration-150 {storageCollapsed ? '-rotate-90' : ''}"
			/>
			<span>Storage</span>
		</button>
		{#if !storageCollapsed}
			<div class="pb-2">
				{#each roots as root (root.name)}
					{#if renamingDrive === root.name}
						<div class="flex items-center gap-1.5 px-3 py-1.5 pl-5">
							<HardDrive size={16} class="shrink-0 opacity-80" />
							<InlineRename
								value={getDriveName(root)}
								onSave={(v) => handleSaveRename(root.name, v)}
								onCancel={handleCancelRename}
								class="flex-1"
							/>
						</div>
					{:else}
						<div
							class="{storageRowClass} {isActive(root.name)
								? navItemActiveClass
								: 'hover:bg-surface-secondary'}"
						>
							<button
								type="button"
								class={storageRowButtonClass}
								onclick={() => handleNavigate(root.name)}
								oncontextmenu={(e) => handleContextMenu(root.name, e)}
							>
								<HardDrive size={16} class="shrink-0 opacity-80" />
								<span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap"
									>{getDriveName(root)}</span
								>
								{#if root.readOnly}
									<Badge>RO</Badge>
								{/if}
							</button>
							<button
								type="button"
								class="shrink-0 rounded p-0.5 opacity-0 transition-opacity group-hover:opacity-100 hover:bg-surface-tertiary"
								onclick={(e) => handleContextMenu(root.name, e)}
								onkeydown={(e) => {
									if (e.key === 'Enter' || e.key === ' ') {
										e.preventDefault();
										handleContextMenu(root.name, e as unknown as MouseEvent);
									}
								}}
								aria-label={`Open menu for ${getDriveName(root)}`}
							>
								<MoreVertical size={14} />
							</button>
						</div>
						{#if contextMenuOpen === root.name}
							<ContextMenu
								x={contextMenuPosition.x}
								y={contextMenuPosition.y}
								items={[
									{ id: 'rename', label: 'Rename', icon: Pencil },
									...(driveNameOverrides[root.name]
										? [{ id: 'reset', label: 'Reset Name', icon: X }]
										: [])
								]}
								onSelect={(id) => handleMenuSelect(root.name, id)}
								onClose={handleMenuClose}
							/>
						{/if}
					{/if}
				{/each}
			</div>
		{/if}
	</div>

	<!-- Places Section -->
	<div class="border-b border-border-secondary">
		<button
			type="button"
			class="flex w-full cursor-pointer items-center gap-1.5 border-none bg-transparent px-3 py-2.5 text-left text-[11px] font-medium tracking-wide text-text-secondary uppercase hover:text-text-primary"
			onclick={() => (placesCollapsed = !placesCollapsed)}
		>
			<ChevronDown
				size={14}
				class="shrink-0 transition-transform duration-150 {placesCollapsed ? '-rotate-90' : ''}"
			/>
			<span>Places</span>
		</button>
		{#if !placesCollapsed}
			<div class="pb-2">
				{#each places as place (place.path)}
					<button
						type="button"
						class="{navItemClass} {isActive(place.path) ? navItemActiveClass : ''}"
						onclick={() => handleNavigate(place.path)}
					>
						<place.icon size={16} class="shrink-0 opacity-80" />
						<span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{place.name}</span>
					</button>
				{/each}
			</div>
		{/if}
	</div>

	<!-- Favorites Section -->
	<div class="border-b border-border-secondary">
		<button
			type="button"
			class="flex w-full cursor-pointer items-center gap-1.5 border-none bg-transparent px-3 py-2.5 text-left text-[11px] font-medium tracking-wide text-text-secondary uppercase hover:text-text-primary"
			onclick={() => (favoritesCollapsed = !favoritesCollapsed)}
		>
			<ChevronDown
				size={14}
				class="shrink-0 transition-transform duration-150 {favoritesCollapsed ? '-rotate-90' : ''}"
			/>
			<span>Favorites</span>
		</button>
		{#if !favoritesCollapsed}
			<div class="pb-2">
				{#if favorites.length === 0}
					<div class="px-5 py-2 text-xs text-text-muted italic">No favorites yet</div>
				{:else}
					{#each favorites as fav (fav.path)}
						<button
							type="button"
							class="{navItemClass} {isActive(fav.path) ? navItemActiveClass : ''}"
							onclick={() => handleNavigate(fav.path)}
						>
							<Star size={16} class="shrink-0 opacity-80" />
							<span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{fav.name}</span>
						</button>
					{/each}
				{/if}
			</div>
		{/if}
	</div>
</aside>
