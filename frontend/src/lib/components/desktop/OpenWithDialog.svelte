<script lang="ts">
	import { Button, Modal, Spinner } from '#lib/components/ui/index.js';
	import { CONFIG } from '#lib/config.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';
	import {
		listDesktopApps,
		openDesktopWithApp,
		type DesktopApplication
	} from '#lib/api/desktopApps.js';

	interface Props {
		open: boolean;
		path: string;
		onclose: () => void;
	}

	let { open, path, onclose }: Props = $props();

	let apps = $state.raw<DesktopApplication[]>([]);
	let mimeType = $state('');
	let selectedAppId = $state('');
	let loading = $state(false);
	let launching = $state(false);
	let errorMessage = $state<string | null>(null);
	let retryToken = $state(0);
	let requestGeneration = 0;

	const showSpinner = createDelayedFlag(() => loading, CONFIG.ui.spinnerDelayMs);
	const fileName = $derived(path.split('/').filter(Boolean).at(-1) ?? path);

	$effect(() => {
		const requestedPath = path;
		const retry = retryToken;
		if (!open) return;

		const generation = ++requestGeneration;
		const controller = new AbortController();
		apps = [];
		mimeType = '';
		selectedAppId = '';
		errorMessage = null;
		loading = true;
		void retry;

		listDesktopApps(requestedPath, controller.signal)
			.then((result) => {
				if (generation !== requestGeneration) return;
				apps = result.apps;
				mimeType = result.mimeType;
				selectedAppId = result.apps.find((app) => app.isDefault)?.id ?? result.apps[0]?.id ?? '';
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
		const appId = selectedAppId;
		if (!appId || launching) return;

		launching = true;
		errorMessage = null;
		try {
			await openDesktopWithApp(path, appId);
			onclose();
		} catch (error) {
			errorMessage = error instanceof Error ? error.message : 'Could not launch this application.';
		} finally {
			launching = false;
		}
	}
</script>

<Modal {open} title="Open “{fileName}” with" {onclose}>
	{#if loading}
		{#if showSpinner.value}
			<div class="flex items-center gap-2 py-2 text-[13px] text-text-secondary" role="status">
				<Spinner size="sm" />Loading applications…
			</div>
		{/if}
	{:else if errorMessage}
		<p class="m-0 flex items-center justify-between gap-3 text-[13px] text-danger" role="alert">
			{errorMessage}
			{#if apps.length === 0}
				<Button variant="secondary" size="sm" onclick={() => (retryToken += 1)}>Try again</Button>
			{/if}
		</p>
	{:else if apps.length === 0}
		<p class="m-0 text-[13px] text-text-secondary">
			No installed application is registered for {mimeType || 'this file'}.
		</p>
	{/if}
	{#if !loading && apps.length > 0}
		<fieldset
			class="m-0 max-h-72 overflow-y-auto rounded border border-border-secondary p-0"
			class:mt-3={errorMessage !== null}
			disabled={launching}
		>
			<legend class="sr-only">Installed applications</legend>
			{#each apps as app (app.id)}
				<label
					class="flex h-8 cursor-pointer items-center gap-3 border-b border-border-secondary px-3 text-[13px] last:border-b-0 hover:bg-surface-secondary has-[:checked]:bg-selection has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-accent"
				>
					<input
						type="radio"
						name="desktop-open-with-app"
						value={app.id}
						checked={selectedAppId === app.id}
						onchange={(event) => (selectedAppId = event.currentTarget.value)}
						class="accent-accent"
					/>
					<span class="min-w-0 flex-1 truncate text-text-primary">{app.name}</span>
					{#if app.isDefault}
						<span class="shrink-0 text-text-muted">Default</span>
					{/if}
				</label>
			{/each}
		</fieldset>
	{/if}

	{#snippet footer()}
		<Button
			variant="primary"
			onclick={confirmOpen}
			disabled={!selectedAppId || loading || launching}
		>
			{launching ? 'Opening…' : 'Open'}
		</Button>
	{/snippet}
</Modal>
