import { createBrowsePaneState, type BrowsePaneState } from './pane.svelte';
import type { BrowseLocationSnapshot } from './paneLocation.svelte';

export interface BrowseTabState {
	id: string;
	panes: BrowsePaneState[];
	focusedPaneId: string;
}

interface BrowseTabsOptions {
	initialLocation: BrowseLocationSnapshot;
	onActiveLocationChange?: (snapshot: BrowseLocationSnapshot, paneId: string) => void;
}

/** Owns tab and split-pane state while pane components mount only for the active tab. */
export function useBrowseTabs(options: BrowseTabsOptions) {
	let tabs = $state.raw<BrowseTabState[]>([]);
	let activeTabId = $state('');
	let nextTabNumber = 1;

	function findTab(tabId: string): BrowseTabState | undefined {
		return tabs.find((tab) => tab.id === tabId);
	}

	function isFocusedPane(tabId: string, paneId: string): boolean {
		const tab = findTab(tabId);
		return activeTabId === tabId && tab?.focusedPaneId === paneId;
	}

	function createPane(tabId: string, paneId: string, location: BrowseLocationSnapshot) {
		return createBrowsePaneState(paneId, location, (snapshot) => {
			if (isFocusedPane(tabId, paneId)) {
				options.onActiveLocationChange?.(snapshot, paneId);
			}
		});
	}

	function createTab(location: BrowseLocationSnapshot): BrowseTabState {
		const tabId = `tab-${nextTabNumber++}`;
		const pane = createPane(tabId, `${tabId}-left`, location);
		return { id: tabId, panes: [pane], focusedPaneId: pane.id };
	}

	const firstTab = createTab(options.initialLocation);
	tabs = [firstTab];
	activeTabId = firstTab.id;

	const activeTab = $derived(findTab(activeTabId));
	const visiblePanes = $derived(activeTab?.panes ?? []);
	const activePane = $derived(
		activeTab?.panes.find((pane) => pane.id === activeTab.focusedPaneId) ?? activeTab?.panes[0]
	);
	const hasSplit = $derived((activeTab?.panes.length ?? 0) > 1);

	function notifyActiveLocation(): void {
		const pane = activePane;
		if (pane) options.onActiveLocationChange?.(pane.location.snapshot, pane.id);
	}

	function updateTab(tabId: string, update: (tab: BrowseTabState) => BrowseTabState): void {
		tabs = tabs.map((tab) => (tab.id === tabId ? update(tab) : tab));
	}

	function newTab(location = activePane?.location.snapshot ?? options.initialLocation): string {
		const tab = createTab(location);
		tabs = [...tabs, tab];
		activeTabId = tab.id;
		notifyActiveLocation();
		return tab.id;
	}

	function closeTab(tabId = activeTabId): void {
		if (tabs.length <= 1 || !findTab(tabId)) return;
		const closingIndex = tabs.findIndex((tab) => tab.id === tabId);
		const nextTabs = tabs.filter((tab) => tab.id !== tabId);
		tabs = nextTabs;
		if (activeTabId === tabId) {
			activeTabId = nextTabs[Math.max(0, closingIndex - 1)].id;
			notifyActiveLocation();
		}
	}

	function activateTab(tabId: string): void {
		if (!findTab(tabId) || activeTabId === tabId) return;
		activeTabId = tabId;
		notifyActiveLocation();
	}

	function cycleTab(direction: -1 | 1): void {
		if (tabs.length < 2) return;
		const currentIndex = tabs.findIndex((tab) => tab.id === activeTabId);
		const nextIndex = (currentIndex + direction + tabs.length) % tabs.length;
		activateTab(tabs[nextIndex].id);
	}

	function focusPane(paneId: string): void {
		const tab = activeTab;
		if (!tab?.panes.some((pane) => pane.id === paneId) || tab.focusedPaneId === paneId) return;
		updateTab(tab.id, (current) => ({ ...current, focusedPaneId: paneId }));
		notifyActiveLocation();
	}

	function toggleSplit(): void {
		const tab = activeTab;
		if (!tab) return;

		if (tab.panes.length > 1) {
			const [left] = tab.panes;
			updateTab(tab.id, (current) => ({ ...current, panes: [left], focusedPaneId: left.id }));
			notifyActiveLocation();
			return;
		}

		const left = tab.panes[0];
		const right = createPane(tab.id, `${tab.id}-right`, left.location.snapshot);
		updateTab(tab.id, (current) => ({
			...current,
			panes: [left, right],
			focusedPaneId: right.id
		}));
		notifyActiveLocation();
	}

	function focusOtherPane(): void {
		const tab = activeTab;
		if (!tab || tab.panes.length < 2) return;
		const nextPane = tab.panes.find((pane) => pane.id !== tab.focusedPaneId);
		if (nextPane) focusPane(nextPane.id);
	}

	return {
		get tabs() {
			return tabs;
		},
		get activeTabId() {
			return activeTabId;
		},
		get activeTab() {
			return activeTab;
		},
		get visiblePanes() {
			return visiblePanes;
		},
		get activePane() {
			return activePane;
		},
		get hasSplit() {
			return hasSplit;
		},
		newTab,
		closeTab,
		activateTab,
		cycleTab,
		focusPane,
		toggleSplit,
		focusOtherPane
	};
}

export type BrowseTabsController = ReturnType<typeof useBrowseTabs>;
