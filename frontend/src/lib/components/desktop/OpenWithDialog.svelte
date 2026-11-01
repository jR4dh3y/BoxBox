<script lang="ts">
	import { Button, Input, Modal, Spinner } from '#lib/components/ui/index.js';
	import { CONFIG } from '#lib/config.js';
	import ApplicationIcon from './ApplicationIcon.svelte';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';
	import {
		listDesktopApps,
		openDesktopWithApp,
		setDesktopDefaultApp,
		type DesktopApplication,
		type DesktopAppsResponse
	} from '#lib/api/desktopApps.js';
	import {
		filterOpenWithChoices,
		initialOpenWithChoice,
		visibleOpenWithChoice,
		withDefaultApplication
	} from '#lib/desktop/openWith.js';

	interface Props {
		open: boolean;
		path: string;
		onclose: () => void;
	}

	let { open, path, onclose }: Props = $props();

	let apps = $state.raw<DesktopAppsResponse>({ mimeType: '', recommended: [], other: [] });
	let search = $state('');
	let selectedAppId = $state('');
	let loading = $state(false);
	let launching = $state(false);
	let useAsDefault = $state(false);
	let errorMessage = $state<string | null>(null);
	let retryToken = $state(0);
	let requestGeneration = 0;

	const showSpinner = createDelayedFlag(() => loading, CONFIG.ui.spinnerDelayMs);
	const fileName = $derived(path.split('/').filter(Boolean).at(-1) ?? path);
	const choices = $derived(filterOpenWithChoices(apps, search));
	const chosenAppId = $derived(visibleOpenWithChoice(choices, selectedAppId));
	const hasApps = $derived(apps.recommended.length > 0 || apps.other.length > 0);

	$effect(() => {
		const requestedPath = path;
		const retry = retryToken;
		if (!open) return;

		const generation = ++requestGeneration;
		const controller = new AbortController();
		apps = { mimeType: '', recommended: [], other: [] };
		search = '';
		selectedAppId = '';
		useAsDefault = false;
		errorMessage = null;
		loading = true;
		void retry;

		listDesktopApps(requestedPath, controller.signal)
			.then((result) => {
				if (generation !== requestGeneration) return;
				apps = result;
				selectedAppId = initialOpenWithChoice(result);
			})
			.catch((error: unknown) => {
				if (generation === requestGeneration && !controller.signal.aborted) {
					errorMessage = error instanceof Error ? error.message : 'Could not load applications.';
				}
			})
			.finally(() => {
				if (generation === requestGeneration) loading = false;
			});

		return () => {
			controller.abort();
			if (generation === requestGeneration) requestGeneration += 1;
		};
	});

	async function confirmOpen() {
		const appId = chosenAppId;
		if (!appId || launching) return;

		launching = true;
		errorMessage = null;
		try {
			if (useAsDefault) {
				await setDesktopDefaultApp(path, appId);
				apps = withDefaultApplication(apps, appId);
				useAsDefault = false;
			}
			await openDesktopWithApp(path, appId);
			onclose();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'Could not launch this application.';
		} finally {
			launching = false;
		}
	}
</script>

{#snippet appRow(app: DesktopApplication)}
	<label
		class="flex h-8 cursor-pointer items-center gap-3 px-3 text-[13px] hover:bg-surface-secondary has-[:checked]:bg-selection has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-accent"
		ondblclick={confirmOpen}
	>
		<input
			type="radio"
			name="desktop-open-with-app"
			value={app.id}
			checked={chosenAppId === app.id}
			onchange={(event) => (selectedAppId = event.currentTarget.value)}
			class="accent-accent"
		/>
		<ApplicationIcon {app} />
		<span class="min-w-0 flex-1 truncate text-text-primary">{app.name}</span>
		{#if app.isDefault}
			<span class="shrink-0 text-text-muted">Default</span>
		{/if}
	</label>
{/snippet}

<Modal {open} title="Open “{fileName}” with" {onclose}>
	{#if loading}
		{#if showSpinner.value}
			<div class="flex items-center gap-2 py-2 text-[13px] text-text-secondary" role="status">
				<Spinner size="sm" />Loading applications…
			</div>
		{/if}
	{:else}
		{#if errorMessage}
			<!-- Reloading also drops an application uninstalled since the list was read. -->
			<p
				class="m-0 mb-3 flex items-center justify-between gap-3 text-[13px] text-danger"
				role="alert"
			>
				{errorMessage}
				<Button variant="secondary" size="sm" onclick={() => (retryToken += 1)}>Reload</Button>
			</p>
		{/if}
		{#if !hasApps}
			{#if !errorMessage}
				<p class="m-0 text-[13px] text-text-secondary">
					No installed application can open this file.
				</p>
			{/if}
		{:else}
			<Input
				type="search"
				bind:value={search}
				placeholder="Search applications"
				onkeydown={(event) => {
					if (event.key === 'Enter') {
						event.preventDefault();
						void confirmOpen();
					}
				}}
			/>
			<fieldset
				class="m-0 mt-2 max-h-72 overflow-y-auto rounded border border-border-secondary p-0"
				disabled={launching}
			>
				<legend class="sr-only">Installed applications</legend>
				<div class="px-3 pt-2 pb-1 text-xs text-text-muted">Recommended</div>
				{#each choices.recommended as app (app.id)}
					{@render appRow(app)}
				{:else}
					<p class="m-0 px-3 pb-2 text-[13px] text-text-secondary">
						{search.trim()
							? 'No recommended application matches.'
							: `No application is registered for ${apps.mimeType || 'this file type'}.`}
					</p>
				{/each}
				{#if choices.other.length > 0}
					<div class="border-t border-border-secondary px-3 pt-2 pb-1 text-xs text-text-muted">
						Other applications
					</div>
					{#each choices.other as app (app.id)}
						{@render appRow(app)}
					{/each}
				{/if}
			</fieldset>
			<label class="mt-3 flex items-center gap-2 text-[13px] text-text-secondary">
				<input
					type="checkbox"
					bind:checked={useAsDefault}
					disabled={!chosenAppId || launching}
					class="accent-accent"
				/>
				Use as default for {apps.mimeType || 'this file type'}
			</label>
		{/if}
	{/if}

	{#snippet footer()}
		<Button variant="primary" onclick={confirmOpen} disabled={!chosenAppId || loading || launching}>
			{launching ? 'Opening…' : 'Open'}
		</Button>
	{/snippet}
</Modal>
