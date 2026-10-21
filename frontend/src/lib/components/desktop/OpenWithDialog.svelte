<script lang="ts">
	import { AlertTriangle, AppWindow, RotateCw } from 'lucide-svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import Modal from '#lib/components/ui/Modal.svelte';
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

<Modal {open} title="Open With" {onclose}>
	<div class="flex flex-col gap-4">
		<p class="text-sm text-text-secondary">
			Choose an application to open <span class="font-medium text-text-primary">{fileName}</span>.
		</p>

		{#if errorMessage}
			<div
				class="flex items-start gap-2 rounded border border-border-secondary bg-surface-secondary px-3 py-2 text-sm text-text-primary"
				role="alert"
			>
				<AlertTriangle size={17} class="mt-0.5 shrink-0 text-danger" aria-hidden="true" />
				<p class="min-w-0 flex-1">{errorMessage}</p>
				<button
					type="button"
					class="shrink-0 rounded p-1 text-text-secondary hover:bg-surface-tertiary hover:text-text-primary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
					aria-label="Try loading applications again"
					onclick={() => (retryToken += 1)}
				>
					<RotateCw size={15} aria-hidden="true" />
				</button>
			</div>
		{/if}

		{#if loading}
			<div
				class="flex flex-col gap-2"
				role="status"
				aria-label="Loading applications"
				aria-live="polite"
				aria-busy="true"
			>
				{#each [0, 1, 2] as row (row)}
					<div class="h-12 animate-pulse rounded bg-surface-secondary"></div>
				{/each}
			</div>
		{:else if apps.length > 0}
			<fieldset
				class="max-h-72 overflow-y-auto rounded border border-border-secondary p-1"
				disabled={launching}
			>
				<legend class="sr-only">Installed applications</legend>
				{#each apps as app (app.id)}
					<label
						class="flex cursor-pointer items-center gap-3 rounded px-3 py-2.5 text-sm transition-colors hover:bg-surface-secondary has-[:checked]:bg-surface-secondary has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-accent"
					>
						<input
							type="radio"
							name="desktop-open-with-app"
							value={app.id}
							checked={selectedAppId === app.id}
							onchange={(event) => (selectedAppId = event.currentTarget.value)}
							class="accent-accent"
						/>
						<AppWindow size={18} class="shrink-0 text-text-secondary" aria-hidden="true" />
						<span class="min-w-0 flex-1 truncate text-text-primary">{app.name}</span>
						{#if app.isDefault}
							<span class="shrink-0 text-xs text-text-secondary">Default</span>
						{/if}
					</label>
				{/each}
			</fieldset>
		{:else if !errorMessage}
			<div
				class="rounded border border-border-secondary bg-surface-secondary px-4 py-5 text-center"
			>
				<AppWindow size={22} class="mx-auto mb-2 text-text-secondary" aria-hidden="true" />
				<p class="text-sm font-medium text-text-primary">No compatible applications found</p>
				<p class="mt-1 text-sm text-text-secondary">
					{#if mimeType}
						No installed app is registered for {mimeType}.
					{:else}
						There are no installed apps registered for this file.
					{/if}
				</p>
			</div>
		{/if}
	</div>

	{#snippet footer()}
		<Button variant="secondary" onclick={onclose} disabled={launching}>Cancel</Button>
		<Button
			variant="primary"
			onclick={confirmOpen}
			disabled={!selectedAppId || loading || launching}
		>
			{launching ? 'Opening…' : 'Open'}
		</Button>
	{/snippet}
</Modal>
