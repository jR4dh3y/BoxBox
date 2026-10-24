<script lang="ts">
	/**
	 * Sidebar component - navigation panel with drives, places, and favorites
	 */
	import {
		ChevronDown,
		Download,
		FileText,
		Folder,
		Computer,
		HardDrive,
		Image,
		Monitor,
		Music,
		Server,
		Star,
		Video,
		X
	} from 'lucide-svelte';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import type { MountPoint } from '#lib/api/files.js';
	import { isLocalDesktop } from '#lib/api/desktop.js';
	import type { BrowsePage } from '#lib/browse/location.svelte.js';
	import { rootLabel } from '#lib/browse/rootLabel.js';
	import type { FileDropTarget } from '#lib/browse/fileDrag.js';
	import LoadError from '#lib/components/LoadError.svelte';

	interface Props {
		currentPath?: string;
		currentPage?: BrowsePage;
		onOpenPage?: (page: BrowsePage) => void;
		/** Drives, places and favorites accept dropped files when set. */
		dropTarget?: FileDropTarget;
		roots?: MountPoint[];
		onNavigate?: (path: string) => void;
		/** Path of the folder under the pointer, or null once it leaves */
		onFolderHover?: (path: string | null) => void;
	}

	let {
		currentPath = '',
		currentPage = 'files',
		onOpenPage,
		dropTarget,
		roots = [],
		onNavigate,
		onFolderHover
	}: Props = $props();

	const desktop = isLocalDesktop();

	const PLACE_ICONS: Record<string, typeof Folder> = {
		desktop: Monitor,
		downloads: Download,
		documents: FileText,
		music: Music,
		pictures: Image,
		videos: Video
	};

	const drives = $derived([
		{ name: rootLabel(), path: '', icon: desktop ? Computer : Server },
		...roots
			.filter((root) => root.kind !== 'place')
			.map((root) => ({
				name: root.serverName
					? `${root.serverName} · ${root.label ?? root.name}`
					: (root.label ?? root.name),
				path: root.name,
				icon: HardDrive
			}))
	]);
	const places = $derived(
		roots
			.filter((root) => root.kind === 'place')
			.map((root) => ({
				// Places are the well-known home folders, so they read as titles.
				name: root.name.charAt(0).toUpperCase() + root.name.slice(1),
				path: root.name,
				icon: PLACE_ICONS[root.name.toLowerCase()] ?? Folder
			}))
	);

	// Desktop-only entries stay out of the web bundle's startup path.
	const desktopLinks = desktop ? import('./desktop/SidebarDesktopLinks.svelte') : null;

	function isActive(path: string): boolean {
		if (currentPage !== 'files') return false;
		if (path === '' && currentPath === '') return true;
		return currentPath.startsWith(path) && path !== '';
	}

	function dragOver(path: string, event: DragEvent): void {
		if (path) dropTarget?.over(event, path);
	}

	function dropOn(path: string, event: DragEvent): void {
		if (path) dropTarget?.drop(event, path);
	}

	function handleNavigate(path: string) {
		onNavigate?.(path);
	}

	// Collapsed sections state
	let drivesCollapsed = $state(false);
	let placesCollapsed = $state(false);
	let favoritesCollapsed = $state(false);

	const navItemClass =
		'w-full flex items-center gap-2.5 py-1.5 px-3 pl-5 bg-transparent border-none text-text-primary text-[13px] cursor-pointer text-left transition-colors duration-100 hover:bg-surface-secondary';
	const navItemActiveClass = 'bg-selection text-white hover:bg-selection-hover';
	const dropTargetClass = 'outline outline-1 -outline-offset-1 outline-accent bg-accent/15';

	function handleUnpin(path: string, event: MouseEvent) {
		event.stopPropagation();
		settingsStore.unpinFavoriteFolder(path);
	}
</script>

<aside
	class="flex w-[220px] min-w-[220px] flex-col overflow-x-hidden overflow-y-auto border-r border-border-secondary bg-surface-primary"
>
	<!-- Drives Section -->
	<div class="border-b border-border-secondary">
		<button
			type="button"
			class="flex w-full cursor-pointer items-center gap-1.5 border-none bg-transparent px-3 py-2.5 text-left text-[11px] font-medium tracking-wide text-text-secondary uppercase hover:text-text-primary"
			onclick={() => (drivesCollapsed = !drivesCollapsed)}
		>
			<ChevronDown
				size={14}
				class="shrink-0 transition-transform duration-150 {drivesCollapsed ? '-rotate-90' : ''}"
			/>
			<span>Drives</span>
		</button>
		{#if !drivesCollapsed}
			<div class="pb-2">
				{#each drives as drive (drive.path)}
					<button
						type="button"
						class="{navItemClass} {isActive(drive.path) ? navItemActiveClass : ''} {drive.path &&
						dropTarget?.overPath === drive.path
							? dropTargetClass
							: ''}"
						onclick={() => handleNavigate(drive.path)}
						ondragover={(event) => dragOver(drive.path, event)}
						ondragleave={(event) => dropTarget?.leave(event)}
						ondrop={(event) => dropOn(drive.path, event)}
						onpointerenter={() => drive.path && onFolderHover?.(drive.path)}
						onpointerleave={() => onFolderHover?.(null)}
					>
						<drive.icon size={16} class="shrink-0 opacity-80" />
						<span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{drive.name}</span>
					</button>
				{/each}
				{#if desktopLinks && onOpenPage}
					{#await desktopLinks then { default: SidebarDesktopLinks }}
						<SidebarDesktopLinks
							pages={['drives', 'servers']}
							{currentPage}
							{onOpenPage}
							itemClass={navItemClass}
							activeClass={navItemActiveClass}
						/>
					{:catch}
						<LoadError label="desktop locations" inline />
					{/await}
				{/if}
			</div>
		{/if}
	</div>

	{#if places.length > 0 || desktopLinks}
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
							class="{navItemClass} {isActive(place.path)
								? navItemActiveClass
								: ''} {dropTarget?.overPath === place.path ? dropTargetClass : ''}"
							onclick={() => handleNavigate(place.path)}
							ondragover={(event) => dragOver(place.path, event)}
							ondragleave={(event) => dropTarget?.leave(event)}
							ondrop={(event) => dropOn(place.path, event)}
							onpointerenter={() => onFolderHover?.(place.path)}
							onpointerleave={() => onFolderHover?.(null)}
						>
							<place.icon size={16} class="shrink-0 opacity-80" />
							<span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap"
								>{place.name}</span
							>
						</button>
					{/each}
					{#if desktopLinks && onOpenPage}
						{#await desktopLinks then { default: SidebarDesktopLinks }}
							<SidebarDesktopLinks
								pages={['trash']}
								{currentPage}
								{onOpenPage}
								itemClass={navItemClass}
								activeClass={navItemActiveClass}
							/>
						{:catch}
							<LoadError label="desktop places" inline />
						{/await}
					{/if}
				</div>
			{/if}
		</div>
	{/if}

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
				{#if settingsStore.current.favoriteFolders.length === 0}
					<div class="px-5 py-2 text-xs text-text-muted italic">No favorites yet</div>
				{:else}
					{#each settingsStore.current.favoriteFolders as fav (fav.path)}
						<div class="group relative">
							<button
								type="button"
								class="{navItemClass} pr-9 {isActive(fav.path)
									? navItemActiveClass
									: ''} {dropTarget?.overPath === fav.path ? dropTargetClass : ''}"
								onclick={() => handleNavigate(fav.path)}
								ondragover={(event) => dragOver(fav.path, event)}
								ondragleave={(event) => dropTarget?.leave(event)}
								ondrop={(event) => dropOn(fav.path, event)}
								onpointerenter={() => onFolderHover?.(fav.path)}
								onpointerleave={() => onFolderHover?.(null)}
							>
								<Star size={16} class="shrink-0 opacity-80" />
								<span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap"
									>{fav.name}</span
								>
							</button>
							<button
								type="button"
								class="absolute top-1 right-2 flex h-6 w-6 items-center justify-center rounded border-none bg-transparent text-text-muted opacity-0 transition-opacity group-hover:opacity-100 hover:bg-surface-secondary hover:text-text-primary"
								onclick={(event) => handleUnpin(fav.path, event)}
								title="Unpin"
								aria-label="Unpin {fav.name}"
							>
								<X size={14} />
							</button>
						</div>
					{/each}
				{/if}
			</div>
		{/if}
	</div>
</aside>
