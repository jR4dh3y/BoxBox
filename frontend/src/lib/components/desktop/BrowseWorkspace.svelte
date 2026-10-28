<script lang="ts">
	import { Columns2, Plus, X } from 'lucide-svelte';
	import BrowseBrowser from '../BrowseBrowser.svelte';
	import Sidebar from '../Sidebar.svelte';
	import RemoteCatalog from './RemoteCatalog.svelte';
	import DesktopUndoShortcut from './DesktopUndoShortcut.svelte';
	import { useBrowseWorkspace } from '#lib/browse/workspace.svelte.js';
	import { parseRemotePath } from '#lib/api/remotePaths.js';
	const workspace = useBrowseWorkspace();
	const { tabs, volumes, setRuntime, volumeAction, keydown } = workspace;
	const active = $derived(workspace.active);
	const runtime = $derived(workspace.runtime);
	const catalog = $derived(workspace.catalog);
</script>

<svelte:window onkeydown={keydown} />
<RemoteCatalog
	atRoot={tabs.visiblePanes.some(
		(pane) => !pane.location.path || /^@remote\/[^/]+$/.test(pane.location.path)
	)}
	onchange={workspace.updateCatalog}
/>
<DesktopUndoShortcut />
<div class="flex h-full min-h-0">
	<Sidebar
		currentPath={active?.location.path ?? ''}
		roots={runtime?.data.roots ?? []}
		onNavigate={(path) => active?.location.navigate(path)}
		open={workspace.sidebarOpen}
		onClose={workspace.closeSidebar}
		volumes={volumes.snapshot.value?.volumes ?? []}
		volumeBusy={volumes.busy}
		onVolumeAction={volumeAction}
		onTrash={() => active?.location.openPage('trash')}
	/>
	<div class="flex min-h-0 min-w-0 flex-1 flex-col">
		<nav
			class="browse-chrome flex h-8 shrink-0 items-center gap-1 overflow-x-auto border-b border-border-secondary px-2"
			aria-label="Browse tabs"
		>
			<button type="button" class="px-2 text-xs md:hidden" onclick={workspace.toggleSidebar}
				>Sidebar</button
			>
			{#each tabs.tabs as tab (tab.id)}
				{@const path = tab.panes[0]?.location.path ?? ''}
				{@const source = parseRemotePath(path)}
				{@const title =
					source.kind === 'remote'
						? `${catalog.servers.find((server) => server.id === source.serverId)?.name ?? 'Deployment'}${source.path ? ' · ' + source.path.split('/').at(-1) : ''}`
						: tab.panes[0]?.location.page === 'trash'
							? 'Trash'
							: path.split('/').at(-1) || 'This PC'}
				<div
					class="flex min-w-0 items-center rounded {tab.id === tabs.activeTabId
						? 'bg-surface-elevated'
						: ''}"
				>
					<button
						type="button"
						class="max-w-44 truncate px-2 py-1 text-xs"
						aria-current={tab.id === tabs.activeTabId ? 'page' : undefined}
						onclick={() => tabs.activateTab(tab.id)}>{title}</button
					>
					{#if tabs.tabs.length > 1}<button
							type="button"
							class="p-1"
							aria-label="Close tab"
							onclick={() => tabs.closeTab(tab.id)}><X size={12} /></button
						>{/if}
				</div>
			{/each}
			<button
				type="button"
				class="p-1 text-text-secondary"
				aria-label="New tab"
				title="New tab (Ctrl+T)"
				onclick={() => tabs.newTab()}><Plus size={14} /></button
			>
			<button
				type="button"
				class="ml-auto p-1 text-text-secondary"
				aria-label="Split pane"
				aria-pressed={tabs.hasSplit}
				title="Split pane (F3)"
				onclick={tabs.toggleSplit}><Columns2 size={14} /></button
			>
		</nav>
		<div class="flex min-h-0 flex-1 divide-x divide-border-secondary">
			{#each tabs.visiblePanes as pane (pane.id)}
				<div
					class="min-h-0 min-w-0 flex-1 {tabs.hasSplit && pane.id === active?.id
						? 'ring-1 ring-accent/40 ring-inset'
						: ''}"
				>
					<BrowseBrowser
						{pane}
						catalog={workspace.combinedCatalog}
						focused={pane.id === active?.id}
						onFocus={() => tabs.focusPane(pane.id)}
						onToggleSidebar={workspace.toggleSidebar}
						onRuntime={(runtime) => setRuntime(pane.id, runtime)}
					/>
				</div>
			{/each}
		</div>
	</div>
</div>
{#if workspace.connectionsOpen}
	{#await import('../settings/SettingsModal.svelte') then { default: SettingsModal }}<SettingsModal
			initialSection="connections"
			onclose={workspace.closeConnections}
		/>{/await}
{/if}
