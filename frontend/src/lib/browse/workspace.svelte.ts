import { afterNavigate, replaceState } from '$app/navigation';
import { page } from '$app/state';
import { SvelteSet, SvelteURL } from 'svelte/reactivity';
import { onMount, untrack } from 'svelte';
import { parseBrowseLocation } from '#lib/browse/paneLocation.svelte.js';
import { useBrowseTabs } from '#lib/browse/tabs.svelte.js';
import type { BrowseRuntime } from '#lib/browse/runtime.js';
import type { RemoteCatalogSnapshot } from '#lib/browse/catalog.js';
import { browseRequests } from '#lib/browse/requests.svelte.js';
import { getBrowseShortcut } from '#lib/browse/paneShortcuts.js';
import { isTextEntryTarget } from '#lib/browse/fileShortcuts.js';
import { settingsStore } from '#lib/stores/settings.svelte.js';
import { VolumesController } from '#lib/desktop/volumes.svelte.js';
import type { DesktopVolume, DesktopVolumeAction } from '#lib/api/desktopVolumes.js';
import { mountedVolumeDrives } from '#lib/desktop/volumeCatalog.js';
import { jobsStore } from '#lib/stores/jobs.svelte.js';
import { openDesktopTab, parseDesktopLaunch, subscribeDesktopLaunches } from './desktopLaunch';
import { toastStore } from '#lib/stores/toast.svelte.js';

export function useBrowseWorkspace() {
	const tabs = useBrowseTabs({
		initialLocation: untrack(() => parseBrowseLocation(page.url)),
		onActiveLocationChange: (snapshot) => {
			const url = new SvelteURL(window.location.href);
			for (const [key, value] of Object.entries({
				path: snapshot.path,
				q: snapshot.searchQuery,
				sort: snapshot.sortBy,
				dir: snapshot.sortDir,
				view: snapshot.viewMode,
				page: snapshot.page === 'files' ? '' : snapshot.page
			})) {
				if (value) url.searchParams.set(key, value);
				else url.searchParams.delete(key);
			}
			url.searchParams.delete('select');
			replaceState(url, page.state);
		}
	});
	let catalog = $state.raw<RemoteCatalogSnapshot>({
		roots: [],
		drives: [],
		servers: [],
		status: { kind: 'loading' },
		states: {}
	});
	let runtimes = $state.raw<Record<string, BrowseRuntime>>({});
	let sidebarOpen = $state(false);
	let connectionsOpen = $state(false);
	const volumes = new VolumesController();
	const active = $derived(tabs.activePane);
	const runtime = $derived(active ? runtimes[active.id] : undefined);
	const volumeDrives = $derived(mountedVolumeDrives(volumes.snapshot.value?.volumes ?? []));
	const combinedCatalog = $derived({
		...catalog,
		roots: [
			...catalog.roots,
			...volumeDrives.map((drive) => ({
				name: drive.name,
				path: drive.path,
				label: drive.label,
				readOnly: drive.readOnly
			}))
		],
		drives: [...catalog.drives, ...volumeDrives]
	});
	function setRuntime(id: string, runtime: BrowseRuntime | null) {
		const next = { ...runtimes };
		if (runtime) next[id] = runtime;
		else delete next[id];
		runtimes = next;
	}
	async function volumeAction(volume: DesktopVolume, action: DesktopVolumeAction) {
		if (await volumes.act(volume, action)) {
			await browseRequests.changed();
			if (action === 'mount') {
				const mounted = volumes.snapshot.value?.volumes.find((item) => item.id === volume.id);
				if (mounted?.browsePath) active?.location.navigate(mounted.browsePath);
			}
		}
		if (volumes.error) toastStore.error(volumes.error);
	}
	function keydown(event: KeyboardEvent) {
		if (
			event.defaultPrevented ||
			document.querySelector('[role="dialog"], dialog[open], [role="menu"]')
		)
			return;
		const action = getBrowseShortcut({
			key: event.key,
			ctrlKey: event.ctrlKey,
			metaKey: event.metaKey,
			altKey: event.altKey,
			shiftKey: event.shiftKey,
			editable: isTextEntryTarget(event.target)
		});
		if (!action || action === 'focus-location') return;
		event.preventDefault();
		switch (action) {
			case 'new-tab':
				tabs.newTab();
				break;
			case 'close-tab':
				tabs.closeTab();
				break;
			case 'next-tab':
				tabs.cycleTab(1);
				break;
			case 'previous-tab':
				tabs.cycleTab(-1);
				break;
			case 'toggle-split':
				tabs.toggleSplit();
				break;
			case 'back':
				active?.location.back();
				break;
			case 'forward':
				active?.location.forward();
				break;
			case 'up':
				active?.location.up();
				break;
			case 'refresh':
				runtime?.data.reload();
				break;
			case 'toggle-hidden':
				settingsStore.set({
					...settingsStore.current,
					showHiddenFiles: !settingsStore.current.showHiddenFiles
				});
				break;
			case 'open-servers':
				connectionsOpen = true;
				break;
		}
	}
	afterNavigate(() => {
		untrack(() => active?.location.restore(parseBrowseLocation(page.url)));
	});
	onMount(() => {
		const initial = parseDesktopLaunch({
			path: active?.location.path,
			select: page.url.searchParams.get('select') ?? undefined
		});
		if (initial?.select) active?.selection.set(new SvelteSet([initial.select]));
		const stopLaunches = subscribeDesktopLaunches(window, (target) => openDesktopTab(tabs, target));
		const stopJobs = jobsStore.connectPolling();
		const request = new AbortController();
		void volumes.load(request.signal);
		const timer = setInterval(() => {
			if (!document.hidden) void volumes.load(request.signal);
		}, 5000);
		return () => {
			stopJobs();
			stopLaunches();
			clearInterval(timer);
			request.abort();
		};
	});

	return {
		tabs,
		volumes,
		setRuntime,
		volumeAction,
		keydown,
		get catalog() {
			return catalog;
		},
		get combinedCatalog() {
			return combinedCatalog;
		},
		get active() {
			return active;
		},
		get runtime() {
			return runtime;
		},
		get sidebarOpen() {
			return sidebarOpen;
		},
		get connectionsOpen() {
			return connectionsOpen;
		},
		updateCatalog(next: RemoteCatalogSnapshot) {
			catalog = next;
		},
		toggleSidebar() {
			sidebarOpen = !sidebarOpen;
		},
		closeSidebar() {
			sidebarOpen = false;
		},
		closeConnections() {
			connectionsOpen = false;
		}
	};
}
