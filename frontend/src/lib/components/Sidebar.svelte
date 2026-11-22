<script lang="ts">
	/**
	 * Sidebar component - navigation panel with drives, places, and favorites
	 */
	import {
		ChevronDown,
		ArrowUpFromLine,
		Trash2,
		Download,
		FileText,
		Folder,
		HardDrive,
		Image,
		Monitor,
		Music,
		LayoutDashboard,
		Star,
		Video,
		X
	} from 'lucide-svelte';
	import DeploymentIcon from './desktop/DeploymentIcon.svelte';
	import { rootLabel } from '#lib/browse/rootLabel.js';
	import type { DesktopVolume, DesktopVolumeAction } from '#lib/api/desktopVolumes.js';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import type { MountPoint } from '#lib/api/files.js';
	import { draggedSources, type FileDropTarget } from '#lib/browse/fileDrag.js';
	import type { Place } from '#lib/browse/placeMenu.js';
	import { hoverPoint, type HoverPoint } from '#lib/browse/hoverPoint.js';

	interface Props {
		currentPath?: string;
		volumes?: DesktopVolume[];
		volumeBusy?: boolean;
		onVolumeAction?: (volume: DesktopVolume, action: DesktopVolumeAction) => void;
		onTrash?: () => void;
		roots?: MountPoint[];
		onNavigate?: (path: string) => void;
		/** Path of the folder under the pointer, or null once it leaves */
		onFolderHover?: (path: string | null) => void;
		/** The pointer is on a drive's row, or null once it leaves or presses; its card waits for it to rest. */
		onDriveHover?: (drive: { path: string; name: string; at: HoverPoint } | null) => void;
		/** On narrow windows the sidebar is a drawer, shown only while this is true. */
		open?: boolean;
		onClose?: () => void;
		/** Drives, places and favorites take dropped files and open when a drag rests on them. */
		dropTarget?: FileDropTarget;
		/** Right-click on an entry: its place menu. */
		onPlaceMenu?: (place: Place, x: number, y: number) => void;
		/** A drive being renamed from its menu. */
		renamingDrive?: string | null;
		onRenameDrive?: (path: string, name: string | null) => void;
	}

	let {
		currentPath = '',
		volumes = [],
		volumeBusy = false,
		onVolumeAction,
		onTrash,
		roots = [],
		onNavigate,
		onFolderHover,
		onDriveHover,
		open = false,
		onClose,
		dropTarget,
		onPlaceMenu,
		renamingDrive = null,
		onRenameDrive
	}: Props = $props();

	function placeMenu(event: MouseEvent, place: Place) {
		event.preventDefault();
		onPlaceMenu?.(place, event.clientX, event.clientY);
	}
	// A favorite that is a drive or a place is listed there already.
	const favorites = $derived(
		settingsStore.current.favoriteFolders.filter(
			(folder) => !roots.some((root) => root.name === folder.path)
		)
	);

	/** Folders dragged from a BoxBox view, which Favorites can pin. */
	function draggedFolders(): { name: string; path: string }[] {
		return (draggedSources() ?? []).filter((source) => source.isDir);
	}
	let pinning = $state(false);

	const PLACE_ICONS: Record<string, typeof Folder> = {
		desktop: Monitor,
		downloads: Download,
		documents: FileText,
		music: Music,
		pictures: Image,
		videos: Video
	};

	const drives = $derived([
		{
			name: rootLabel(),
			path: '',
			serverIcon: undefined,
			icon: LayoutDashboard
		},
		...roots
			.filter((root) => root.kind !== 'place')
			.map((root) => ({
				// A name given on the drives page or here.
				name: settingsStore.current.driveNameOverrides[root.name] || (root.label ?? root.name),
				path: root.name,
				serverIcon: root.serverIcon,
				icon: HardDrive
			}))
	]);
	const places = $derived(
		roots
			.filter((root) => root.kind === 'place')
			.map((root) => ({
				name: root.label ?? root.name,
				path: root.name,
				icon: PLACE_ICONS[root.name.toLowerCase()] ?? Folder
			}))
	);

	/** Overview's row is the drives page, which shows every card already. */
	function restOnDrive(drive: { path: string; name: string }, event: PointerEvent) {
		if (drive.path) onDriveHover?.({ path: drive.path, name: drive.name, at: hoverPoint(event) });
	}

	function isActive(path: string): boolean {
		if (path === '') return currentPath === '';
		// "media" owns "media/movies" but not "media2".
		return currentPath === path || currentPath.startsWith(`${path}/`);
	}

	function handleNavigate(path: string) {
		onNavigate?.(path);
		onClose?.();
	}

	function handleKeydown(event: KeyboardEvent) {
		if (open && event.key === 'Escape') onClose?.();
	}

	// Collapsed sections state
	let drivesCollapsed = $state(false);
	let placesCollapsed = $state(false);
	let favoritesCollapsed = $state(false);

	const navItemClass =
		'w-full flex items-center gap-2.5 py-1.5 px-3 pl-5 bg-transparent border-none text-text-primary text-[13px] cursor-pointer text-left transition-colors duration-100 hover:bg-surface-secondary';
	const navItemActiveClass = 'bg-selection text-white hover:bg-selection-hover';

	function handleUnpin(path: string, event: MouseEvent) {
		event.stopPropagation();
		settingsStore.unpinFavoriteFolder(path);
	}
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div class="fixed inset-0 z-[55] bg-black/40 md:hidden" onclick={onClose}></div>
{/if}

<aside
	class="browse-chrome {open
		? 'fixed inset-y-0 left-0 z-[60] flex shadow-2xl'
		: 'hidden'} w-[220px] min-w-[220px] flex-col overflow-x-hidden overflow-y-auto border-r border-border-secondary bg-surface-primary md:static md:z-auto md:flex md:shadow-none"
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
					{#if renamingDrive !== null && renamingDrive === drive.path}
						<div class="px-3 py-1">
							<!-- Renaming is rare, so its field loads on first use. -->
							{#await import('./ui/InlineRename.svelte') then { default: InlineRename }}
								<InlineRename
									value={drive.name}
									onSave={(name) => onRenameDrive?.(drive.path, name || null)}
									onCancel={() => onRenameDrive?.(drive.path, drive.name)}
								/>
							{/await}
						</div>
					{:else}
						<button
							type="button"
							class="{navItemClass} {isActive(drive.path) ? navItemActiveClass : ''}"
							onclick={() => handleNavigate(drive.path)}
							oncontextmenu={(event) => placeMenu(event, { kind: 'drive', path: drive.path })}
							class:ring-1={dropTarget?.overPath === drive.path}
							class:ring-inset={dropTarget?.overPath === drive.path}
							class:ring-accent={dropTarget?.overPath === drive.path}
							ondragover={(event) => {
								if (drive.path && dropTarget?.over(event, drive.path)) event.stopPropagation();
							}}
							ondragleave={(event) => dropTarget?.leave(event)}
							ondrop={(event) => {
								if (drive.path) dropTarget?.drop(event, drive.path);
							}}
							onpointerenter={(event) => {
								if (drive.path) onFolderHover?.(drive.path);
								restOnDrive(drive, event);
							}}
							onpointermove={(event) => restOnDrive(drive, event)}
							onpointerleave={() => {
								onFolderHover?.(null);
								onDriveHover?.(null);
							}}
							onpointerdown={() => onDriveHover?.(null)}
						>
							{#if drive.serverIcon}<DeploymentIcon icon={drive.serverIcon} />{:else}<drive.icon
									size={16}
									class="shrink-0 opacity-80"
								/>{/if}
							<span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap"
								>{drive.name}</span
							>
						</button>
					{/if}
				{/each}
			</div>
		{/if}
	</div>

	{#if places.length > 0}
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
							oncontextmenu={(event) => placeMenu(event, { kind: 'drive', path: place.path })}
							class:ring-1={dropTarget?.overPath === place.path}
							class:ring-inset={dropTarget?.overPath === place.path}
							class:ring-accent={dropTarget?.overPath === place.path}
							ondragover={(event) => {
								if (place.path && dropTarget?.over(event, place.path)) event.stopPropagation();
							}}
							ondragleave={(event) => dropTarget?.leave(event)}
							ondrop={(event) => {
								if (place.path) dropTarget?.drop(event, place.path);
							}}
							onpointerenter={() => onFolderHover?.(place.path)}
							onpointerleave={() => onFolderHover?.(null)}
						>
							<place.icon size={16} class="shrink-0 opacity-80" />
							<span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap capitalize"
								>{place.name}</span
							>
						</button>
					{/each}
				</div>
			{/if}
		</div>
	{/if}

	{#if volumes.length > 0}
		<div class="border-b border-border-secondary py-2">
			<p class="px-3 pb-1 text-[11px] font-medium text-text-secondary uppercase">Devices</p>
			{#each volumes as volume (volume.id)}
				<div class="flex items-center pr-2">
					<button
						type="button"
						class={navItemClass}
						disabled={volumeBusy || (!volume.browsePath && !volume.canMount)}
						onclick={() =>
							volume.mounted && volume.browsePath
								? handleNavigate(volume.browsePath)
								: onVolumeAction?.(volume, 'mount')}
						><HardDrive size={16} /><span class="truncate">{volume.name}</span></button
					>
					{#if volume.canEject || volume.canUnmount}<button
							type="button"
							class="text-xs text-text-secondary"
							disabled={volumeBusy}
							aria-label="Eject {volume.name}"
							onclick={() => onVolumeAction?.(volume, volume.canEject ? 'eject' : 'unmount')}
							><ArrowUpFromLine size={14} /></button
						>{/if}
				</div>
			{/each}
		</div>
	{/if}
	{#if onTrash}<button type="button" class={navItemClass} onclick={onTrash}
			><Trash2 size={16} />Trash</button
		>{/if}

	<!-- Favorites Section: a folder dropped here, not on a favorite, is pinned. -->
	<div
		class="border-b border-border-secondary {pinning
			? 'bg-accent/10 ring-1 ring-accent ring-inset'
			: ''}"
		role="region"
		aria-label="Favorites"
		ondragover={(event) => {
			if (draggedFolders().length === 0) return;
			event.preventDefault();
			// Drags allow copy or move; pinning copies nothing, but must use one of them.
			if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
			pinning = true;
		}}
		ondragleave={(event) => {
			const area = event.currentTarget;
			if (event.relatedTarget instanceof Node && area.contains(event.relatedTarget)) return;
			pinning = false;
		}}
		ondrop={(event) => {
			pinning = false;
			const folders = draggedFolders();
			if (folders.length === 0) return;
			event.preventDefault();
			for (const folder of folders)
				if (!roots.some((root) => root.name === folder.path))
					settingsStore.pinFavoriteFolder(folder);
		}}
	>
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
					<div class="px-5 py-2 text-xs text-text-muted">No favorites yet</div>
				{:else}
					{#each favorites as fav (fav.path)}
						<div class="group relative">
							<button
								type="button"
								class="{navItemClass} pr-9 {isActive(fav.path) ? navItemActiveClass : ''}"
								onclick={() => handleNavigate(fav.path)}
								oncontextmenu={(event) => placeMenu(event, { kind: 'favorite', path: fav.path })}
								class:ring-1={dropTarget?.overPath === fav.path}
								class:ring-inset={dropTarget?.overPath === fav.path}
								class:ring-accent={dropTarget?.overPath === fav.path}
								ondragover={(event) => {
									if (fav.path && dropTarget?.over(event, fav.path)) event.stopPropagation();
								}}
								ondragleave={(event) => dropTarget?.leave(event)}
								ondrop={(event) => {
									if (fav.path) dropTarget?.drop(event, fav.path);
								}}
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
