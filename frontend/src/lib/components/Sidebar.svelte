<script lang="ts">
	/**
	 * Sidebar component - navigation panel with drives, places, and favorites
	 */
	import {
		ChevronDown,
		Download,
		FileText,
		Folder,
		HardDrive,
		Image,
		Monitor,
		Music,
		Server,
		Star,
		Video,
		X
	} from 'lucide-svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import type { MountPoint } from '$lib/api/files';

	interface Props {
		currentPath?: string;
		roots?: MountPoint[];
		onNavigate?: (path: string) => void;
		/** Path of the folder under the pointer, or null once it leaves */
		onFolderHover?: (path: string | null) => void;
	}

	let { currentPath = '', roots = [], onNavigate, onFolderHover }: Props = $props();

	const PLACE_ICONS: Record<string, typeof Folder> = {
		desktop: Monitor,
		downloads: Download,
		documents: FileText,
		music: Music,
		pictures: Image,
		videos: Video
	};

	const drives = $derived([
		{ name: 'This Server', path: '', icon: Server },
		...roots
			.filter((root) => root.kind !== 'place')
			.map((root) => ({ name: root.name, path: root.name, icon: HardDrive }))
	]);
	const places = $derived(
		roots
			.filter((root) => root.kind === 'place')
			.map((root) => ({
				name: root.name,
				path: root.name,
				icon: PLACE_ICONS[root.name.toLowerCase()] ?? Folder
			}))
	);

	function isActive(path: string): boolean {
		if (path === '' && currentPath === '') return true;
		return currentPath.startsWith(path) && path !== '';
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
						class="{navItemClass} {isActive(drive.path) ? navItemActiveClass : ''}"
						onclick={() => handleNavigate(drive.path)}
						onpointerenter={() => drive.path && onFolderHover?.(drive.path)}
						onpointerleave={() => onFolderHover?.(null)}
					>
						<drive.icon size={16} class="shrink-0 opacity-80" />
						<span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap">{drive.name}</span>
					</button>
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
							onpointerenter={() => onFolderHover?.(place.path)}
							onpointerleave={() => onFolderHover?.(null)}
						>
							<place.icon size={16} class="shrink-0 opacity-80" />
							<span class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap"
								>{place.name}</span
							>
						</button>
					{/each}
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
								class="{navItemClass} pr-9 {isActive(fav.path) ? navItemActiveClass : ''}"
								onclick={() => handleNavigate(fav.path)}
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
