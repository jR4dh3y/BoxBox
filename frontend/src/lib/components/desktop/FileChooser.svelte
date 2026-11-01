<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { FileChooser } from '#lib/desktop/chooser.svelte.js';
	import FileList from '#lib/components/FileList.svelte';
	import ChooserLocations from './ChooserLocations.svelte';
	import { Button, Input } from '#lib/components/ui/index.js';
	import { ArrowUp, Folder, HardDrive, Search, X, MapPin } from 'lucide-svelte';
	import { isLocalDesktop } from '#lib/api/desktop.js';
	let searchInput: HTMLInputElement | undefined;
	const chooser = new FileChooser(page.url.searchParams.get('id') ?? '');
	onMount(() => {
		if (isLocalDesktop()) void chooser.initialize();
		return () => chooser.dispose();
	});
</script>

<svelte:window
	onkeydown={(event) => {
		if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'f') {
			event.preventDefault();
			searchInput?.focus();
			searchInput?.select();
		}
	}}
/>

<svelte:head><title>{chooser.request?.title || 'Choose a file'} · BoxBox</title></svelte:head>

{#if !isLocalDesktop()}
	<p class="p-6 text-text-primary">The file chooser is available in BoxBox Desktop.</p>
{:else}
	<main
		aria-labelledby="chooser-title"
		class="flex h-dvh flex-col bg-surface-primary text-[13px] text-text-primary"
	>
		<h1 id="chooser-title" class="sr-only">{chooser.request?.title || 'Choose a file'}</h1>
		<div class="flex min-h-0 flex-1">
			<nav
				aria-label="Places"
				class="w-44 shrink-0 overflow-auto border-r border-border-secondary bg-surface-secondary p-2 max-sm:w-28"
			>
				{#each chooser.roots as root (root.name)}
					<button
						class="flex w-full items-center gap-2 rounded px-2 py-2 text-left hover:bg-surface-tertiary focus-visible:outline-2 focus-visible:outline-accent"
						class:bg-selection={chooser.view === 'files' && chooser.path === root.name}
						onclick={() => chooser.open(root.name)}
					>
						{#if root.name === 'root'}<HardDrive size={16} />{:else}<Folder size={16} />{/if}<span
							class="truncate">{root.label || root.name}</span
						>
					</button>
				{/each}
				<button
					type="button"
					class="mt-2 flex w-full items-center gap-2 rounded px-2 py-2 text-left hover:bg-surface-tertiary focus-visible:outline-2 focus-visible:outline-accent"
					class:bg-selection={chooser.view === 'locations'}
					onclick={() => chooser.showLocations()}
					><MapPin size={16} class="shrink-0" /><span>Other locations</span></button
				>
			</nav>
			<section aria-label="Files" class="flex min-w-0 flex-1 flex-col">
				{#if chooser.view === 'locations'}
					<ChooserLocations {chooser} />
				{:else}
					<div
						class="flex min-h-12 shrink-0 items-center gap-3 border-b border-border-secondary p-2"
					>
						<form
							class="flex min-w-0 flex-1 items-center gap-2"
							onsubmit={(event) => {
								event.preventDefault();
								void chooser.open(chooser.location);
							}}
						>
							<Button
								variant="ghost"
								title="Parent folder"
								disabled={!chooser.path.includes('/')}
								onclick={() => chooser.open(chooser.path.slice(0, chooser.path.lastIndexOf('/')))}
								><ArrowUp size={16} /></Button
							>
							<label class="sr-only" for="chooser-location">Location</label><input
								id="chooser-location"
								class="min-w-0 flex-1 rounded border border-border-secondary bg-surface-primary px-3 py-2 outline-accent"
								title="Enter a location and press Enter"
								bind:value={chooser.location}
							/>
						</form>
						<form
							role="search"
							class="relative w-48 max-w-[45%] shrink-0"
							onsubmit={(event) => event.preventDefault()}
						>
							<label class="sr-only" for="chooser-search">Search this folder</label>
							<Search
								size={16}
								class="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-text-secondary"
							/>
							<input
								id="chooser-search"
								bind:this={searchInput}
								type="search"
								placeholder="Search this folder"
								class="w-full rounded border border-border-secondary bg-surface-primary py-2 pr-8 pl-8 outline-accent [&::-webkit-search-cancel-button]:appearance-none"
								value={chooser.query}
								oninput={(event) => chooser.search(event.currentTarget.value)}
								onkeydown={(event) => {
									if (event.key === 'Escape') {
										event.preventDefault();
										chooser.search('');
									}
								}}
							/>
							{#if chooser.query}
								<button
									type="button"
									aria-label="Clear search"
									class="absolute top-1/2 right-1 -translate-y-1/2 rounded p-1 text-text-secondary hover:bg-surface-tertiary focus-visible:outline-2 focus-visible:outline-accent"
									onclick={() => {
										chooser.search('');
										searchInput?.focus();
									}}><X size={16} /></button
								>
							{/if}
						</form>
					</div>
					<div class="min-h-0 flex-1 overflow-auto">
						<FileList
							items={chooser.visible}
							emptyMessage={chooser.query.trim()
								? 'No matching files or folders'
								: 'This folder is empty'}
							viewKey={chooser.path}
							selectedPaths={chooser.selected}
							isLoading={chooser.loading}
							selectionCheckboxes={true}
							onSelectionChange={(paths) => chooser.select(paths)}
							onItemClick={(item) => chooser.choose(item)}
							sortBy={chooser.sortBy}
							sortDir={chooser.sortDir}
							onSortChange={(field, dir) => {
								chooser.sortBy = field;
								chooser.sortDir = dir;
							}}
							readOnly={true}
						/>
					</div>
				{/if}
				{#if chooser.error}<p role="alert" class="px-4 py-2 text-danger">{chooser.error}</p>{/if}
				{#if chooser.done}<p role="status" class="px-4 py-2 text-text-secondary">
						Selection complete. This window will close.
					</p>{/if}
			</section>
		</div>
		<footer
			class="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-3 border-t border-border-secondary px-4 py-3"
		>
			{#if chooser.request?.kind === 'save'}<label class="flex flex-1 items-center gap-2"
					>Name <Input bind:value={chooser.name} placeholder="File name" /></label
				>{/if}
			{#if chooser.request?.kind === 'save-files'}<p>
					Save {chooser.request.files.length} files in this folder
				</p>{/if}
			{#if chooser.request?.filters.length}
				<label class="flex items-center gap-2"
					>File type <select
						class="max-w-64 rounded border border-border-secondary bg-surface-primary p-2"
						bind:value={chooser.filter}
					>
						{#each chooser.request.filters as filter, index (index)}<option value={index}
								>{filter.name}</option
							>{/each}
					</select></label
				>
			{/if}
			{#each chooser.request?.choices ?? [] as choice (choice.id)}
				{#if choice.options.length}
					<label class="flex items-center gap-2"
						>{choice.label}<select
							class="rounded border border-border-secondary bg-surface-primary p-2"
							bind:value={chooser.choices[choice.id]}
							>{#each choice.options as option (option.id)}<option value={option.id}
									>{option.label}</option
								>{/each}</select
						></label
					>
				{:else}<label class="flex items-center gap-2"
						><input
							type="checkbox"
							class="accent-accent"
							checked={chooser.choices[choice.id] === 'true'}
							onchange={(event) =>
								(chooser.choices[choice.id] = String(event.currentTarget.checked))}
						/>{choice.label}</label
					>{/if}
			{/each}
			<label class="flex items-center gap-2"
				><input type="checkbox" class="accent-accent" bind:checked={chooser.hidden} />Show hidden
				files</label
			>
			<div class="ml-auto flex shrink-0 items-center gap-2">
				<Button variant="secondary" onclick={() => chooser.cancel()}>Cancel</Button>
				<Button
					variant="primary"
					onclick={() => chooser.accept()}
					disabled={chooser.loading ||
						chooser.view === 'locations' ||
						chooser.busy ||
						!chooser.request ||
						chooser.done ||
						(chooser.request.kind === 'open' &&
							!chooser.request.directory &&
							chooser.selected.size === 0) ||
						(chooser.request.kind === 'save' && !chooser.name)}
				>
					{chooser.replace ? 'Replace' : chooser.request?.acceptLabel || 'Open'}
				</Button>
			</div>
		</footer>
	</main>
{/if}
